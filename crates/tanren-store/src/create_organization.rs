use sea_orm::{ActiveModelTrait, DatabaseConnection, DatabaseTransaction, Set, TransactionTrait};
use uuid::Uuid;

use crate::StoreError;
use crate::entity;
use crate::records::{
    MembershipRecord, NewOrganization, OrganizationRecord, org_permission_to_str,
};
use crate::traits::{
    CreateOrganizationAtomicOutput, CreateOrganizationAtomicRequest, CreateOrganizationError,
};

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
        organization,
        membership_id,
        bootstrap_permissions,
        now,
    } = request;

    let org_record = insert_organization_in_txn(txn, &organization).await?;
    let membership_record = insert_membership_in_txn(
        txn,
        membership_id,
        organization.creator_account_id,
        organization.id,
        now,
    )
    .await?;

    for perm in &bootstrap_permissions {
        insert_permission_grant_in_txn(
            txn,
            organization.id,
            organization.creator_account_id,
            *perm,
            now,
        )
        .await?;
    }

    Ok(CreateOrganizationAtomicOutput {
        organization: org_record,
        membership: membership_record,
    })
}

async fn insert_organization_in_txn(
    txn: &DatabaseTransaction,
    org: &NewOrganization,
) -> Result<OrganizationRecord, CreateOrganizationError> {
    let model = entity::organizations::ActiveModel {
        id: Set(org.id.as_uuid()),
        canonical_name: Set(org.canonical_name.clone()),
        display_name: Set(org.display_name.clone()),
        creator_account_id: Set(org.creator_account_id.as_uuid()),
        created_at: Set(org.created_at),
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
    OrganizationRecord::try_from(inserted).map_err(CreateOrganizationError::Store)
}

async fn insert_membership_in_txn(
    txn: &DatabaseTransaction,
    membership_id: tanren_identity_policy::MembershipId,
    account_id: tanren_identity_policy::AccountId,
    org_id: tanren_identity_policy::OrgId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<MembershipRecord, CreateOrganizationError> {
    let model = entity::memberships::ActiveModel {
        id: Set(membership_id.as_uuid()),
        account_id: Set(account_id.as_uuid()),
        org_id: Set(org_id.as_uuid()),
        created_at: Set(now),
    };
    model.insert(txn).await.map_err(StoreError::from)?;
    Ok(MembershipRecord {
        id: membership_id,
        account_id,
        org_id,
        created_at: now,
    })
}

async fn insert_permission_grant_in_txn(
    txn: &DatabaseTransaction,
    org_id: tanren_identity_policy::OrgId,
    account_id: tanren_identity_policy::AccountId,
    permission: tanren_identity_policy::OrgPermission,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), CreateOrganizationError> {
    let model = entity::organization_permission_grants::ActiveModel {
        id: Set(Uuid::now_v7()),
        org_id: Set(org_id.as_uuid()),
        account_id: Set(account_id.as_uuid()),
        permission: Set(org_permission_to_str(permission).to_owned()),
        granted_at: Set(now),
    };
    model.insert(txn).await.map_err(StoreError::from)?;
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
