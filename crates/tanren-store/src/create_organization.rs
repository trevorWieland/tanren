//! `SeaORM`-backed implementation of the atomic organization-creation
//! flow. Lives in its own module so the lib.rs core file stays under
//! the workspace per-file line budget.
//!
//! Wraps the insert organization + insert admin membership + append
//! events sequence in one DB transaction. Failure on any step rolls
//! the whole flow back.

use sea_orm::{ActiveModelTrait, DatabaseConnection, DatabaseTransaction, Set, TransactionTrait};
use tanren_identity_policy::{AccountId, MembershipId, OrgId};
use uuid::Uuid;

use crate::entity;
use crate::traits::{
    CreateOrganizationAtomicOutput, CreateOrganizationAtomicRequest, CreateOrganizationError,
    CreateOrganizationEventContext,
};
use crate::{MembershipRecord, OrganizationRecord, StoreError};

pub(crate) async fn run(
    conn: &DatabaseConnection,
    request: CreateOrganizationAtomicRequest,
) -> Result<CreateOrganizationAtomicOutput, CreateOrganizationError> {
    conn.transaction::<_, CreateOrganizationAtomicOutput, CreateOrganizationError>(|txn| {
        Box::pin(async move { run_in_txn(txn, request).await })
    })
    .await
    .map_err(map_transaction_error)
}

async fn run_in_txn(
    txn: &DatabaseTransaction,
    request: CreateOrganizationAtomicRequest,
) -> Result<CreateOrganizationAtomicOutput, CreateOrganizationError> {
    let CreateOrganizationAtomicRequest {
        org_id,
        name,
        name_normalized,
        now,
        creator_account_id,
        membership_id,
        permissions,
        events_builder,
    } = request;

    let org_record = insert_organization_in_txn(txn, org_id, &name, &name_normalized, now).await?;
    let membership_record = insert_membership_in_txn(
        txn,
        membership_id,
        creator_account_id,
        org_id,
        permissions,
        now,
    )
    .await?;
    append_success_events_in_txn(
        txn,
        events_builder,
        &CreateOrganizationEventContext {
            org_id,
            creator_account_id,
            now,
        },
    )
    .await?;

    Ok(CreateOrganizationAtomicOutput {
        organization: org_record,
        membership: membership_record,
    })
}

async fn insert_organization_in_txn(
    txn: &DatabaseTransaction,
    org_id: OrgId,
    name: &str,
    name_normalized: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<OrganizationRecord, CreateOrganizationError> {
    let model = entity::organizations::ActiveModel {
        id: Set(org_id.as_uuid()),
        name: Set(name.to_owned()),
        name_normalized: Set(name_normalized.to_owned()),
        created_at: Set(now),
    };
    let inserted = match model.insert(txn).await {
        Ok(a) => a,
        Err(err) => {
            let lower = err.to_string().to_lowercase();
            if lower.contains("unique") || lower.contains("duplicate") {
                return Err(CreateOrganizationError::DuplicateName);
            }
            return Err(StoreError::from(err).into());
        }
    };
    Ok(OrganizationRecord::from(inserted))
}

async fn insert_membership_in_txn(
    txn: &DatabaseTransaction,
    membership_id: MembershipId,
    creator_account_id: AccountId,
    org_id: OrgId,
    permissions: u32,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<MembershipRecord, CreateOrganizationError> {
    let model = entity::memberships::ActiveModel {
        id: Set(membership_id.as_uuid()),
        account_id: Set(creator_account_id.as_uuid()),
        org_id: Set(org_id.as_uuid()),
        created_at: Set(now),
        permissions: Set(i64::from(permissions)),
    };
    let inserted = model.insert(txn).await.map_err(StoreError::from)?;
    Ok(MembershipRecord::from(inserted))
}

async fn append_success_events_in_txn(
    txn: &DatabaseTransaction,
    events_builder: crate::traits::CreateOrganizationEventsBuilder,
    ctx: &CreateOrganizationEventContext,
) -> Result<(), CreateOrganizationError> {
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
    err: sea_orm::TransactionError<CreateOrganizationError>,
) -> CreateOrganizationError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            CreateOrganizationError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}
