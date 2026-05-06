//! `SeaORM`-backed implementation of the atomic project-registration
//! flow. Lives in its own module so the lib.rs core file stays under
//! the workspace per-file line budget.
//!
//! Wraps the insert project + upsert active-project selection + append
//! success-events sequence in one DB transaction. Failure on any step
//! rolls the whole flow back. Duplicate repository identity constraint
//! violations are mapped to [`RegisterProjectError::DuplicateRepository`].

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use tanren_identity_policy::OrgId;
use uuid::Uuid;

use crate::entity;
use crate::traits::{
    RegisterProjectAtomicRequest, RegisterProjectError, RegisterProjectEventContext,
    RegisterProjectEventsBuilder, RegisterProjectOutput,
};
use crate::{ActiveProjectRecord, ProjectRecord, StoreError};

pub(crate) async fn run(
    conn: &DatabaseConnection,
    request: RegisterProjectAtomicRequest,
) -> Result<RegisterProjectOutput, RegisterProjectError> {
    conn.transaction::<_, RegisterProjectOutput, RegisterProjectError>(|txn| {
        Box::pin(async move { run_in_txn(txn, request).await })
    })
    .await
    .map_err(map_transaction_error)
}

async fn run_in_txn(
    txn: &DatabaseTransaction,
    request: RegisterProjectAtomicRequest,
) -> Result<RegisterProjectOutput, RegisterProjectError> {
    let RegisterProjectAtomicRequest {
        new,
        now,
        events_builder,
    } = request;

    let project_model = entity::projects::ActiveModel {
        id: Set(new.id.as_uuid()),
        name: Set(new.name),
        repository_id: Set(new.repository_id.as_uuid()),
        owner_account_id: Set(new.owner_account_id.as_uuid()),
        owner_org_id: Set(new.owner_org_id.map(OrgId::as_uuid)),
        repository_identity: Set(new.repository_identity.clone()),
        repository_url: Set(new.repository_url),
        created_at: Set(new.created_at),
    };
    let inserted = match project_model.insert(txn).await {
        Ok(m) => m,
        Err(err) => {
            let lower = err.to_string().to_lowercase();
            if lower.contains("unique") || lower.contains("duplicate") {
                return Err(RegisterProjectError::DuplicateRepository);
            }
            return Err(RegisterProjectError::Store(StoreError::from(err)));
        }
    };
    let project_record = ProjectRecord::from(inserted);

    upsert_active_project_in_txn(txn, new.owner_account_id.as_uuid(), new.id.as_uuid(), now)
        .await?;

    let ctx = RegisterProjectEventContext {
        project_id: new.id,
        repository_id: new.repository_id,
        owner_account_id: new.owner_account_id,
        now,
    };
    append_success_events_in_txn(txn, events_builder, &ctx).await?;

    Ok(RegisterProjectOutput {
        project: project_record,
        active_project: ActiveProjectRecord {
            account_id: ctx.owner_account_id,
            project_id: ctx.project_id,
            selected_at: now,
        },
    })
}

async fn upsert_active_project_in_txn(
    txn: &DatabaseTransaction,
    account_uuid: Uuid,
    project_uuid: Uuid,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), RegisterProjectError> {
    entity::active_projects::Entity::delete_many()
        .filter(entity::active_projects::Column::AccountId.eq(account_uuid))
        .exec(txn)
        .await
        .map_err(StoreError::from)?;

    let model = entity::active_projects::ActiveModel {
        account_id: Set(account_uuid),
        project_id: Set(project_uuid),
        selected_at: Set(now),
    };
    model.insert(txn).await.map_err(StoreError::from)?;
    Ok(())
}

async fn append_success_events_in_txn(
    txn: &DatabaseTransaction,
    events_builder: RegisterProjectEventsBuilder,
    ctx: &RegisterProjectEventContext,
) -> Result<(), RegisterProjectError> {
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
    err: sea_orm::TransactionError<RegisterProjectError>,
) -> RegisterProjectError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            RegisterProjectError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}
