use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, EntityTrait};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::{SignpostId, SpecId, TaskId};

use crate::Store;
use crate::entity::{methodology_signpost_spec, methodology_task_spec};
use crate::errors::StoreResult;

impl Store {
    /// Read one task->spec projection row by task id.
    pub async fn load_methodology_task_spec_projection(
        &self,
        task_id: TaskId,
    ) -> StoreResult<Option<SpecId>> {
        let row = methodology_task_spec::Entity::find_by_id(task_id.into_uuid())
            .one(self.conn())
            .await?;
        Ok(row.map(|row| SpecId::from_uuid(row.spec_id)))
    }

    /// Upsert one task->spec projection row.
    pub async fn upsert_methodology_task_spec_projection(
        &self,
        task_id: TaskId,
        spec_id: SpecId,
    ) -> StoreResult<()> {
        upsert_task_spec_projection(self.conn(), task_id, spec_id).await
    }

    /// Read one signpost->spec projection row by signpost id.
    pub async fn load_methodology_signpost_spec_projection(
        &self,
        signpost_id: SignpostId,
    ) -> StoreResult<Option<SpecId>> {
        let row = methodology_signpost_spec::Entity::find_by_id(signpost_id.into_uuid())
            .one(self.conn())
            .await?;
        Ok(row.map(|row| SpecId::from_uuid(row.spec_id)))
    }

    /// Upsert one signpost->spec projection row.
    pub async fn upsert_methodology_signpost_spec_projection(
        &self,
        signpost_id: SignpostId,
        spec_id: SpecId,
    ) -> StoreResult<()> {
        upsert_signpost_spec_projection(self.conn(), signpost_id, spec_id).await
    }
}

pub(crate) async fn upsert_spec_lookup_projection_txn(
    txn: &DatabaseTransaction,
    event: &MethodologyEvent,
) -> StoreResult<()> {
    match event {
        MethodologyEvent::TaskCreated(e) => {
            upsert_task_spec_projection(txn, e.task.id, e.task.spec_id).await?;
        }
        MethodologyEvent::SignpostAdded(e) => {
            upsert_signpost_spec_projection(txn, e.signpost.id, e.signpost.spec_id).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn upsert_task_spec_projection<C: sea_orm::ConnectionTrait>(
    conn: &C,
    task_id: TaskId,
    spec_id: SpecId,
) -> StoreResult<()> {
    let now = Utc::now();
    let task_id = task_id.into_uuid();
    let spec_id = spec_id.into_uuid();
    match methodology_task_spec::Entity::find_by_id(task_id)
        .one(conn)
        .await?
    {
        Some(row) => {
            let mut active = methodology_task_spec::ActiveModel::from(row);
            active.spec_id = Set(spec_id);
            active.updated_at = Set(now);
            active.update(conn).await?;
        }
        None => {
            methodology_task_spec::Entity::insert(methodology_task_spec::ActiveModel {
                task_id: Set(task_id),
                spec_id: Set(spec_id),
                updated_at: Set(now),
            })
            .exec(conn)
            .await?;
        }
    }
    Ok(())
}

async fn upsert_signpost_spec_projection<C: sea_orm::ConnectionTrait>(
    conn: &C,
    signpost_id: SignpostId,
    spec_id: SpecId,
) -> StoreResult<()> {
    let now = Utc::now();
    let signpost_id = signpost_id.into_uuid();
    let spec_id = spec_id.into_uuid();
    match methodology_signpost_spec::Entity::find_by_id(signpost_id)
        .one(conn)
        .await?
    {
        Some(row) => {
            let mut active = methodology_signpost_spec::ActiveModel::from(row);
            active.spec_id = Set(spec_id);
            active.updated_at = Set(now);
            active.update(conn).await?;
        }
        None => {
            methodology_signpost_spec::Entity::insert(methodology_signpost_spec::ActiveModel {
                signpost_id: Set(signpost_id),
                spec_id: Set(spec_id),
                updated_at: Set(now),
            })
            .exec(conn)
            .await?;
        }
    }
    Ok(())
}
