//! `SeaORM`-backed implementation of the atomic invitation-acceptance
//! flow. Lives in its own module so the lib.rs core file stays under
//! the workspace per-file line budget.
//!
//! Wraps the consume + insert account + insert membership + insert
//! session + append events sequence in one DB transaction. Failure on
//! any step rolls the whole flow back so the invitation row stays
//! pending and the user can retry — closing the previous gap where a
//! transient failure after the consume burned the token without
//! producing an account.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use tanren_identity_policy::{MembershipId, OrgId, SessionToken, ValidationError};
use uuid::Uuid;

use crate::entity;
use crate::traits::{
    AcceptInvitationAtomicOutput, AcceptInvitationAtomicRequest, AcceptInvitationError,
    AcceptInvitationEventContext, AcceptInvitationEventsBuilder,
};
use crate::{AccountRecord, NewAccount, SessionRecord, StoreError};

pub(crate) async fn run(
    conn: &DatabaseConnection,
    request: AcceptInvitationAtomicRequest,
) -> Result<AcceptInvitationAtomicOutput, AcceptInvitationError> {
    conn.transaction::<_, AcceptInvitationAtomicOutput, AcceptInvitationError>(|txn| {
        Box::pin(async move { run_in_txn(txn, request).await })
    })
    .await
    .map_err(map_transaction_error)
}

async fn run_in_txn(
    txn: &DatabaseTransaction,
    request: AcceptInvitationAtomicRequest,
) -> Result<AcceptInvitationAtomicOutput, AcceptInvitationError> {
    let AcceptInvitationAtomicRequest {
        token,
        now,
        account,
        membership_id,
        session_token,
        session_expires_at,
        events_builder,
    } = request;

    let inviting_org_id = consume_invitation_in_txn(txn, token.as_str(), now).await?;
    let account_record = insert_account_in_txn(txn, &account, inviting_org_id).await?;
    insert_membership_in_txn(
        txn,
        membership_id,
        account_record.id.as_uuid(),
        inviting_org_id,
        now,
    )
    .await?;
    let session_record = insert_session_in_txn(
        txn,
        session_token,
        account_record.id.as_uuid(),
        now,
        session_expires_at,
    )
    .await?;
    append_success_events_in_txn(
        txn,
        events_builder,
        &AcceptInvitationEventContext {
            account_id: account_record.id,
            identifier: account_record.identifier.clone(),
            token,
            joined_org: inviting_org_id,
            now,
        },
    )
    .await?;

    Ok(AcceptInvitationAtomicOutput {
        account: account_record,
        session: session_record,
        joined_org: inviting_org_id,
    })
}

/// Conditional UPDATE on the invitation row + disambiguation read.
/// Mirrors `Store::consume_invitation` exactly; running inside a
/// transaction means the disambiguation `find_by_id` sees a consistent
/// view even under concurrent acceptance.
async fn consume_invitation_in_txn(
    txn: &DatabaseTransaction,
    token: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<OrgId, AcceptInvitationError> {
    let token_owned = token.to_owned();
    let result = entity::invitations::Entity::update_many()
        .col_expr(
            entity::invitations::Column::ConsumedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .filter(entity::invitations::Column::Token.eq(token_owned.clone()))
        .filter(entity::invitations::Column::ConsumedAt.is_null())
        .filter(entity::invitations::Column::ExpiresAt.gt(now))
        .exec(txn)
        .await
        .map_err(StoreError::from)?;

    if result.rows_affected == 1 {
        let row = entity::invitations::Entity::find_by_id(token_owned)
            .one(txn)
            .await
            .map_err(StoreError::from)?
            .ok_or_else(|| StoreError::DataInvariant {
                column: "invitation_token",
                cause: ValidationError::InvitationTokenEmpty,
            })?;
        return Ok(OrgId::new(row.inviting_org_id));
    }

    let existing = entity::invitations::Entity::find_by_id(token_owned)
        .one(txn)
        .await
        .map_err(StoreError::from)?;
    Err(match existing {
        None => AcceptInvitationError::InvitationNotFound,
        Some(row) if row.consumed_at.is_some() => AcceptInvitationError::InvitationAlreadyConsumed,
        Some(row) if row.expires_at <= now => AcceptInvitationError::InvitationExpired,
        Some(_) => AcceptInvitationError::InvitationAlreadyConsumed,
    })
}

/// Insert the new account row. Caller's `account.org_id` is ignored —
/// the inviting-org id read from the consumed invitation row is the
/// source of truth.
async fn insert_account_in_txn(
    txn: &DatabaseTransaction,
    account: &NewAccount,
    inviting_org_id: OrgId,
) -> Result<AccountRecord, AcceptInvitationError> {
    let account_model = entity::accounts::ActiveModel {
        id: Set(account.id.as_uuid()),
        identifier: Set(account.identifier.as_str().to_owned()),
        display_name: Set(account.display_name.clone()),
        password_phc: Set(account.password_phc.clone()),
        created_at: Set(account.created_at),
        org_id: Set(Some(inviting_org_id.as_uuid())),
    };
    let inserted = match account_model.insert(txn).await {
        Ok(a) => a,
        Err(err) => {
            let lower = err.to_string().to_lowercase();
            if lower.contains("unique") || lower.contains("duplicate") {
                return Err(AcceptInvitationError::DuplicateIdentifier);
            }
            return Err(StoreError::from(err).into());
        }
    };
    AccountRecord::try_from(inserted).map_err(AcceptInvitationError::Store)
}

async fn insert_membership_in_txn(
    txn: &DatabaseTransaction,
    membership_id: MembershipId,
    account_uuid: Uuid,
    inviting_org_id: OrgId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), AcceptInvitationError> {
    let model = entity::memberships::ActiveModel {
        id: Set(membership_id.as_uuid()),
        account_id: Set(account_uuid),
        org_id: Set(inviting_org_id.as_uuid()),
        created_at: Set(now),
        permissions: Set(0i64),
    };
    model.insert(txn).await.map_err(StoreError::from)?;
    Ok(())
}

async fn insert_session_in_txn(
    txn: &DatabaseTransaction,
    session_token: SessionToken,
    account_uuid: Uuid,
    now: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<SessionRecord, AcceptInvitationError> {
    let model = entity::account_sessions::ActiveModel {
        token: Set(session_token.expose_secret().to_owned()),
        account_id: Set(account_uuid),
        created_at: Set(now),
        expires_at: Set(expires_at),
    };
    model.insert(txn).await.map_err(StoreError::from)?;
    Ok(SessionRecord {
        token: session_token,
        account_id: tanren_identity_policy::AccountId::new(account_uuid),
        created_at: now,
        expires_at,
    })
}

async fn append_success_events_in_txn(
    txn: &DatabaseTransaction,
    events_builder: AcceptInvitationEventsBuilder,
    ctx: &AcceptInvitationEventContext,
) -> Result<(), AcceptInvitationError> {
    for payload in (events_builder)(ctx) {
        let model = entity::events::ActiveModel {
            id: Set(Uuid::now_v7()),
            occurred_at: Set(ctx.now),
            payload: Set(payload),
        };
        model.insert(txn).await.map_err(StoreError::from)?;
    }
    Ok(())
}

/// Translate the `SeaORM` [`sea_orm::TransactionError`] envelope back
/// into the domain-shaped [`AcceptInvitationError`]. The `Connection`
/// arm only fires for the begin/commit/rollback steps themselves; it
/// surfaces as a `Store` variant so callers can distinguish it from
/// in-flight taxonomy failures.
fn map_transaction_error(
    err: sea_orm::TransactionError<AcceptInvitationError>,
) -> AcceptInvitationError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            AcceptInvitationError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}
