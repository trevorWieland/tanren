use sea_orm::{
    ActiveModelTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::entity;
use crate::traits::{
    ConnectProjectAtomicOutput, ConnectProjectAtomicRequest, DisconnectProjectAtomicOutput,
    DisconnectProjectAtomicRequest, DisconnectProjectError,
};
use crate::{NewProject, ProjectRecord, StoreError};

pub(crate) async fn connect_atomic(
    conn: &DatabaseConnection,
    request: ConnectProjectAtomicRequest,
) -> Result<ConnectProjectAtomicOutput, StoreError> {
    conn.transaction::<_, ConnectProjectAtomicOutput, StoreError>(|txn| {
        Box::pin(async move {
            append_events_in_txn(txn, &request.events, request.now).await?;
            let record = insert_project_in_txn(txn, &request.project).await?;
            Ok(ConnectProjectAtomicOutput { project: record })
        })
    })
    .await
    .map_err(map_store_tx_error)
}

pub(crate) async fn disconnect_atomic(
    conn: &DatabaseConnection,
    request: DisconnectProjectAtomicRequest,
) -> Result<DisconnectProjectAtomicOutput, DisconnectProjectError> {
    conn.transaction::<_, DisconnectProjectAtomicOutput, DisconnectProjectError>(|txn| {
        Box::pin(async move {
            append_events_in_txn(txn, &request.events, request.now)
                .await
                .map_err(DisconnectProjectError::Store)?;
            let record = disconnect_project_in_txn(txn, request.project_id, request.now).await?;
            Ok(DisconnectProjectAtomicOutput { project: record })
        })
    })
    .await
    .map_err(map_disconnect_tx_error)
}

async fn insert_project_in_txn(
    txn: &DatabaseTransaction,
    new: &NewProject,
) -> Result<ProjectRecord, StoreError> {
    let model = entity::projects::ActiveModel {
        id: Set(new.id.as_uuid()),
        org_id: Set(new.org_id.as_uuid()),
        name: Set(new.name.clone()),
        provider_connection_id: Set(new.provider_connection_id.as_uuid()),
        resource_id: Set(new.resource_id.clone()),
        display_ref: Set(new.display_ref.clone()),
        connected_at: Set(new.connected_at),
        disconnected_at: Set(None),
    };
    let inserted = model.insert(txn).await?;
    Ok(ProjectRecord::from(inserted))
}

async fn disconnect_project_in_txn(
    txn: &DatabaseTransaction,
    project_id: tanren_identity_policy::ProjectId,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<ProjectRecord, DisconnectProjectError> {
    let row = entity::projects::Entity::find_by_id(project_id.as_uuid())
        .one(txn)
        .await
        .map_err(StoreError::from)?
        .ok_or(DisconnectProjectError::NotFound)?;

    let mut active: entity::projects::ActiveModel = row.into();
    active.disconnected_at = Set(Some(now));
    let updated = active.update(txn).await.map_err(StoreError::from)?;
    Ok(ProjectRecord::from(updated))
}

async fn append_events_in_txn(
    txn: &DatabaseTransaction,
    events: &[serde_json::Value],
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), StoreError> {
    for payload in events {
        let model = entity::events::ActiveModel {
            id: Set(Uuid::now_v7()),
            occurred_at: Set(now),
            payload: Set(payload.clone()),
        };
        model.insert(txn).await?;
    }
    Ok(())
}

fn map_store_tx_error(err: sea_orm::TransactionError<StoreError>) -> StoreError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => StoreError::from(db_err),
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}

fn map_disconnect_tx_error(
    err: sea_orm::TransactionError<DisconnectProjectError>,
) -> DisconnectProjectError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            DisconnectProjectError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}
