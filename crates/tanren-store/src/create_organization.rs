use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, Set,
    TransactionTrait,
};
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
    let key = request.request_id.clone();
    let account_id = request.organization.creator_account_id.as_uuid();
    let canonical_name = request.organization.canonical_name.clone();

    if let Some(cached) = check_idempotency(conn, &key, account_id, &canonical_name).await? {
        return Ok(cached);
    }

    conn.transaction::<_, CreateOrganizationAtomicOutput, CreateOrganizationError>(|txn| {
        Box::pin(async move {
            if let Some(cached) = check_idempotency(txn, &key, account_id, &canonical_name).await? {
                return Ok(cached);
            }

            let output = run_creation_in_txn(txn, request).await?;

            insert_idempotency_row_in_txn(
                txn,
                &key,
                account_id,
                &canonical_name,
                &output,
                output.organization.created_at,
            )
            .await?;

            Ok(output)
        })
    })
    .await
    .map_err(map_transaction_error)
}

async fn check_idempotency<C>(
    db: &C,
    key: &str,
    account_id: Uuid,
    canonical_name: &str,
) -> Result<Option<CreateOrganizationAtomicOutput>, CreateOrganizationError>
where
    C: ConnectionTrait,
{
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let row = entity::organization_create_idempotency::Entity::find()
        .filter(entity::organization_create_idempotency::Column::RequestId.eq(key))
        .one(db)
        .await
        .map_err(StoreError::from)?;

    let Some(row) = row else {
        return Ok(None);
    };

    if row.account_id != account_id || row.canonical_name != canonical_name {
        return Err(CreateOrganizationError::IdempotencyConflict);
    }

    let output: CreateOrganizationAtomicOutput = serde_json::from_value(row.response_json)
        .map_err(|_e| StoreError::DataInvariant {
            column: "response_json",
            cause: tanren_identity_policy::ValidationError::EmptyOrganizationName,
        })?;

    Ok(Some(output))
}

async fn run_creation_in_txn(
    txn: &DatabaseTransaction,
    request: CreateOrganizationAtomicRequest,
) -> Result<CreateOrganizationAtomicOutput, CreateOrganizationError> {
    let CreateOrganizationAtomicRequest {
        organization,
        membership_id,
        bootstrap_permissions,
        now,
        event_payload,
        ..
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

    insert_event_in_txn(txn, &event_payload, now).await?;

    Ok(CreateOrganizationAtomicOutput {
        organization: org_record,
        membership: membership_record,
    })
}

async fn insert_event_in_txn(
    txn: &DatabaseTransaction,
    payload: &serde_json::Value,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), CreateOrganizationError> {
    let model = entity::events::ActiveModel {
        id: Set(Uuid::now_v7()),
        occurred_at: Set(now),
        payload: Set(payload.clone()),
    };
    model.insert(txn).await.map_err(StoreError::from)?;
    Ok(())
}

async fn insert_idempotency_row_in_txn(
    txn: &DatabaseTransaction,
    key: &str,
    account_id: Uuid,
    canonical_name: &str,
    output: &CreateOrganizationAtomicOutput,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), CreateOrganizationError> {
    let response_json = serde_json::to_value(output).map_err(|_e| StoreError::DataInvariant {
        column: "response_json",
        cause: tanren_identity_policy::ValidationError::EmptyOrganizationName,
    })?;
    let model = entity::organization_create_idempotency::ActiveModel {
        request_id: Set(key.to_owned()),
        account_id: Set(account_id),
        canonical_name: Set(canonical_name.to_owned()),
        response_json: Set(response_json),
        created_at: Set(now),
    };
    model.insert(txn).await.map_err(StoreError::from)?;
    Ok(())
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
