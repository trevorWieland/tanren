//! `SeaORM`-backed implementation of atomic organization creation.
//! Keeps `lib.rs` below the workspace file-size budget.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, Set, TransactionTrait,
};
use tanren_identity_policy::{AccountId, MembershipId, OrgId, OrganizationPermission};
use uuid::Uuid;

use crate::entity;
use crate::traits::{
    CreateOrganizationAtomicOutput, CreateOrganizationAtomicRequest, CreateOrganizationError,
    CreateOrganizationEventContext, CreateOrganizationEventsBuilder,
    LastOrganizationAdminGuardError,
};
use crate::{OrganizationRecord, StoreError, organization_permission_key};

const ORGANIZATION_ADMIN_PERMISSIONS: [OrganizationPermission; 5] = [
    OrganizationPermission::Invite,
    OrganizationPermission::ManageAccess,
    OrganizationPermission::Configure,
    OrganizationPermission::SetPolicy,
    OrganizationPermission::Delete,
];

pub(crate) async fn run(
    conn: &DatabaseConnection,
    request: CreateOrganizationAtomicRequest,
) -> Result<CreateOrganizationAtomicOutput, CreateOrganizationError> {
    let replay_account_id = request.creator_account_id;
    let replay_name = request.name.clone();
    let replay_key = request.idempotency_key.clone();
    if let Some(key) = replay_key.as_deref() {
        if let Some(replayed) =
            find_idempotent_replay_for_key(conn, replay_account_id, key, &replay_name).await?
        {
            return Ok(replayed);
        }
    }

    let result = conn
        .transaction::<_, CreateOrganizationAtomicOutput, CreateOrganizationError>(|txn| {
            Box::pin(async move { run_in_txn(txn, request).await })
        })
        .await
        .map_err(map_transaction_error);

    match result {
        Ok(output) => Ok(output),
        Err(CreateOrganizationError::DuplicateName) if replay_key.is_some() => {
            match find_idempotent_replay_for_key(
                conn,
                replay_account_id,
                replay_key.as_deref().unwrap_or_default(),
                &replay_name,
            )
            .await?
            {
                Some(replayed) => Ok(replayed),
                None => Err(CreateOrganizationError::DuplicateName),
            }
        }
        Err(CreateOrganizationError::IdempotencyConflict) if replay_key.is_some() => {
            match find_idempotent_replay_for_key(
                conn,
                replay_account_id,
                replay_key.as_deref().unwrap_or_default(),
                &replay_name,
            )
            .await?
            {
                Some(replayed) => Ok(replayed),
                None => Err(CreateOrganizationError::IdempotencyConflict),
            }
        }
        Err(err) => Err(err),
    }
}

async fn run_in_txn(
    txn: &DatabaseTransaction,
    request: CreateOrganizationAtomicRequest,
) -> Result<CreateOrganizationAtomicOutput, CreateOrganizationError> {
    let CreateOrganizationAtomicRequest {
        organization_id,
        name,
        creator_account_id,
        creator_membership_id,
        now,
        idempotency_key,
        events_builder,
    } = request;

    if let Some(key) = idempotency_key.as_deref() {
        insert_idempotency_claim_in_txn(txn, creator_account_id, key, organization_id, &name, now)
            .await?;
    }

    let organization =
        insert_organization_in_txn(txn, organization_id, &name, creator_account_id, now).await?;
    insert_creator_membership_in_txn(
        txn,
        creator_membership_id,
        creator_account_id,
        organization_id,
        now,
    )
    .await?;
    let granted_permissions =
        insert_creator_admin_grants_in_txn(txn, creator_account_id, organization_id, now).await?;
    append_success_events_in_txn(
        txn,
        events_builder,
        &CreateOrganizationEventContext {
            organization: organization.clone(),
            creator_account_id,
            creator_membership_id,
            granted_permissions: granted_permissions.clone(),
            now,
        },
    )
    .await?;

    Ok(CreateOrganizationAtomicOutput {
        organization,
        granted_permissions,
        initial_project_count: 0,
    })
}

async fn find_idempotent_replay_for_key(
    conn: &DatabaseConnection,
    account_id: AccountId,
    idempotency_key: &str,
    expected_name: &tanren_identity_policy::OrganizationName,
) -> Result<Option<CreateOrganizationAtomicOutput>, CreateOrganizationError> {
    let row = entity::organization_create_idempotency::Entity::find_by_id((
        account_id.as_uuid(),
        idempotency_key.to_owned(),
    ))
    .one(conn)
    .await
    .map_err(StoreError::from)?;
    let Some(row) = row else {
        return Ok(None);
    };

    if row.organization_name != expected_name.as_str() {
        return Err(CreateOrganizationError::IdempotencyConflict);
    }

    let organization_row = entity::organizations::Entity::find_by_id(row.organization_id)
        .one(conn)
        .await
        .map_err(StoreError::from)?
        .ok_or_else(|| {
            CreateOrganizationError::Store(StoreError::Database(sea_orm::DbErr::Custom(
                "idempotency record references missing organization".to_owned(),
            )))
        })?;
    let organization =
        OrganizationRecord::try_from(organization_row).map_err(CreateOrganizationError::Store)?;
    Ok(Some(CreateOrganizationAtomicOutput {
        organization,
        granted_permissions: ORGANIZATION_ADMIN_PERMISSIONS.to_vec(),
        initial_project_count: 0,
    }))
}

async fn insert_organization_in_txn(
    txn: &DatabaseTransaction,
    organization_id: OrgId,
    name: &tanren_identity_policy::OrganizationName,
    creator_account_id: AccountId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<OrganizationRecord, CreateOrganizationError> {
    let model = entity::organizations::ActiveModel {
        id: Set(organization_id.as_uuid()),
        name: Set(name.as_str().to_owned()),
        created_by_account_id: Set(creator_account_id.as_uuid()),
        created_at: Set(now),
    };

    let inserted = match model.insert(txn).await {
        Ok(row) => row,
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

async fn insert_idempotency_claim_in_txn(
    txn: &DatabaseTransaction,
    account_id: AccountId,
    idempotency_key: &str,
    organization_id: OrgId,
    organization_name: &tanren_identity_policy::OrganizationName,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), CreateOrganizationError> {
    let model = entity::organization_create_idempotency::ActiveModel {
        account_id: Set(account_id.as_uuid()),
        key: Set(idempotency_key.to_owned()),
        organization_id: Set(organization_id.as_uuid()),
        organization_name: Set(organization_name.as_str().to_owned()),
        created_at: Set(now),
    };
    match model.insert(txn).await {
        Ok(_) => Ok(()),
        Err(err) => {
            let lower = err.to_string().to_ascii_lowercase();
            if lower.contains("unique") || lower.contains("duplicate") {
                return Err(CreateOrganizationError::IdempotencyConflict);
            }
            Err(StoreError::from(err).into())
        }
    }
}

async fn insert_creator_membership_in_txn(
    txn: &DatabaseTransaction,
    membership_id: MembershipId,
    creator_account_id: AccountId,
    organization_id: OrgId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), CreateOrganizationError> {
    let model = entity::memberships::ActiveModel {
        id: Set(membership_id.as_uuid()),
        account_id: Set(creator_account_id.as_uuid()),
        org_id: Set(organization_id.as_uuid()),
        created_at: Set(now),
    };
    model.insert(txn).await.map_err(StoreError::from)?;
    Ok(())
}

async fn insert_creator_admin_grants_in_txn(
    txn: &DatabaseTransaction,
    creator_account_id: AccountId,
    organization_id: OrgId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<OrganizationPermission>, CreateOrganizationError> {
    for permission in ORGANIZATION_ADMIN_PERMISSIONS {
        let model = entity::organization_permission_grants::ActiveModel {
            id: Set(Uuid::now_v7()),
            org_id: Set(organization_id.as_uuid()),
            account_id: Set(creator_account_id.as_uuid()),
            permission: Set(organization_permission_key(permission).to_owned()),
            granted_by_account_id: Set(creator_account_id.as_uuid()),
            created_at: Set(now),
        };
        model.insert(txn).await.map_err(StoreError::from)?;
    }

    Ok(ORGANIZATION_ADMIN_PERMISSIONS.to_vec())
}

async fn append_success_events_in_txn(
    txn: &DatabaseTransaction,
    events_builder: CreateOrganizationEventsBuilder,
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

pub(crate) async fn has_permission(
    conn: &DatabaseConnection,
    account_id: AccountId,
    org_id: OrgId,
    permission: OrganizationPermission,
) -> Result<bool, StoreError> {
    let has_membership = entity::memberships::Entity::find()
        .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::memberships::Column::OrgId.eq(org_id.as_uuid()))
        .one(conn)
        .await?
        .is_some();
    if !has_membership {
        return Ok(false);
    }

    let row = entity::organization_permission_grants::Entity::find()
        .filter(entity::organization_permission_grants::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::organization_permission_grants::Column::OrgId.eq(org_id.as_uuid()))
        .filter(
            entity::organization_permission_grants::Column::Permission
                .eq(organization_permission_key(permission)),
        )
        .one(conn)
        .await?;

    Ok(row.is_some())
}

pub(crate) async fn enforce_not_last_admin_holder(
    conn: &DatabaseConnection,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<(), LastOrganizationAdminGuardError> {
    let mut orphaned_permissions = Vec::new();

    for permission in ORGANIZATION_ADMIN_PERMISSIONS {
        if !has_permission(conn, account_id, org_id, permission).await? {
            continue;
        }

        let holder_count = entity::organization_permission_grants::Entity::find()
            .filter(entity::organization_permission_grants::Column::OrgId.eq(org_id.as_uuid()))
            .filter(
                entity::organization_permission_grants::Column::Permission
                    .eq(organization_permission_key(permission)),
            )
            .count(conn)
            .await
            .map_err(StoreError::from)?;

        if holder_count <= 1 {
            orphaned_permissions.push(permission);
        }
    }

    if orphaned_permissions.is_empty() {
        return Ok(());
    }

    Err(LastOrganizationAdminGuardError::LastAdminHolder {
        permissions: orphaned_permissions,
    })
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
