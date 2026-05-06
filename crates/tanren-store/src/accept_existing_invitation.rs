use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use tanren_identity_policy::{AccountId, MembershipId, OrgId, OrgPermissions, ValidationError};
use uuid::Uuid;

use crate::entity;
use crate::traits::{
    AcceptExistingInvitationError, AcceptExistingInvitationEventContext,
    AcceptExistingInvitationEventsBuilder, AcceptExistingInvitationOutput,
    AcceptExistingInvitationRequest,
};
use crate::{MembershipRecord, StoreError};

pub(crate) async fn run(
    conn: &DatabaseConnection,
    request: AcceptExistingInvitationRequest,
) -> Result<AcceptExistingInvitationOutput, AcceptExistingInvitationError> {
    conn.transaction::<_, AcceptExistingInvitationOutput, AcceptExistingInvitationError>(|txn| {
        Box::pin(async move { run_in_txn(txn, request).await })
    })
    .await
    .map_err(map_transaction_error)
}

async fn run_in_txn(
    txn: &DatabaseTransaction,
    request: AcceptExistingInvitationRequest,
) -> Result<AcceptExistingInvitationOutput, AcceptExistingInvitationError> {
    let AcceptExistingInvitationRequest {
        token,
        account_id,
        identifier,
        membership_id,
        now,
        events_builder,
    } = request;

    let consumed =
        consume_invitation_in_txn(txn, token.as_str(), account_id, identifier.as_str(), now)
            .await?;

    let membership_record = insert_membership_in_txn(
        txn,
        membership_id,
        account_id,
        consumed.inviting_org_id,
        consumed.org_permissions.as_ref(),
        now,
    )
    .await?;

    append_success_events_in_txn(
        txn,
        events_builder,
        &AcceptExistingInvitationEventContext {
            account_id,
            identifier,
            token,
            joined_org: consumed.inviting_org_id,
            now,
        },
    )
    .await?;

    Ok(AcceptExistingInvitationOutput {
        membership: membership_record,
        joined_org: consumed.inviting_org_id,
    })
}

struct ConsumedExistingInvitation {
    inviting_org_id: OrgId,
    org_permissions: Option<OrgPermissions>,
}

async fn consume_invitation_in_txn(
    txn: &DatabaseTransaction,
    token: &str,
    account_id: AccountId,
    identifier: &str,
    now: DateTime<Utc>,
) -> Result<ConsumedExistingInvitation, AcceptExistingInvitationError> {
    let token_owned = token.to_owned();
    let result = entity::invitations::Entity::update_many()
        .col_expr(
            entity::invitations::Column::ConsumedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .col_expr(
            entity::invitations::Column::ConsumedBy,
            sea_orm::sea_query::Expr::value(Some(account_id.as_uuid())),
        )
        .filter(entity::invitations::Column::Token.eq(token_owned.clone()))
        .filter(entity::invitations::Column::ConsumedAt.is_null())
        .filter(entity::invitations::Column::ExpiresAt.gt(now))
        .filter(entity::invitations::Column::RevokedAt.is_null())
        .filter(entity::invitations::Column::TargetIdentifier.eq(identifier))
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
        let org_permissions = row
            .org_permissions
            .as_deref()
            .map(crate::parse_db_org_permissions)
            .transpose()?;
        return Ok(ConsumedExistingInvitation {
            inviting_org_id: OrgId::new(row.inviting_org_id),
            org_permissions,
        });
    }

    let existing = entity::invitations::Entity::find_by_id(token_owned)
        .one(txn)
        .await
        .map_err(StoreError::from)?;
    Err(match existing {
        None => AcceptExistingInvitationError::InvitationNotFound,
        Some(row) if row.consumed_at.is_some() => {
            AcceptExistingInvitationError::InvitationAlreadyConsumed
        }
        Some(row) if row.expires_at <= now => AcceptExistingInvitationError::InvitationExpired,
        Some(row) if row.revoked_at.is_some() => AcceptExistingInvitationError::InvitationRevoked,
        Some(row) if row.target_identifier.as_deref() != Some(identifier) => {
            AcceptExistingInvitationError::WrongAccount
        }
        Some(_) => AcceptExistingInvitationError::InvitationAlreadyConsumed,
    })
}

async fn insert_membership_in_txn(
    txn: &DatabaseTransaction,
    membership_id: MembershipId,
    account_id: AccountId,
    org_id: OrgId,
    org_permissions: Option<&OrgPermissions>,
    now: DateTime<Utc>,
) -> Result<MembershipRecord, AcceptExistingInvitationError> {
    let model = entity::memberships::ActiveModel {
        id: Set(membership_id.as_uuid()),
        account_id: Set(account_id.as_uuid()),
        org_id: Set(org_id.as_uuid()),
        created_at: Set(now),
        org_permissions: Set(org_permissions.map(|p| p.as_str().to_owned())),
    };
    let inserted = match model.insert(txn).await {
        Ok(m) => m,
        Err(err) => {
            let lower = err.to_string().to_lowercase();
            if lower.contains("unique") || lower.contains("duplicate") {
                return Err(AcceptExistingInvitationError::AlreadyMember);
            }
            return Err(StoreError::from(err).into());
        }
    };
    MembershipRecord::try_from(inserted).map_err(AcceptExistingInvitationError::Store)
}

async fn append_success_events_in_txn(
    txn: &DatabaseTransaction,
    events_builder: AcceptExistingInvitationEventsBuilder,
    ctx: &AcceptExistingInvitationEventContext,
) -> Result<(), AcceptExistingInvitationError> {
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

fn map_transaction_error(
    err: sea_orm::TransactionError<AcceptExistingInvitationError>,
) -> AcceptExistingInvitationError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            AcceptExistingInvitationError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}
