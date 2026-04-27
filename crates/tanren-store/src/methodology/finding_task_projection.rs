use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::{FindingId, SpecId, TaskId};

use crate::Store;
use crate::entity::methodology_task_finding;
use crate::errors::StoreResult;

const SQLITE_CHUNK_SIZE: usize = 800;

impl Store {
    /// Read all finding ids projected for one `(spec, task)` pair.
    ///
    /// # Errors
    /// Returns a store/database error on query failures.
    pub async fn load_methodology_finding_ids_for_task_projection(
        &self,
        spec_id: SpecId,
        task_id: TaskId,
    ) -> StoreResult<Vec<FindingId>> {
        let rows = methodology_task_finding::Entity::find()
            .filter(methodology_task_finding::Column::SpecId.eq(spec_id.into_uuid()))
            .filter(methodology_task_finding::Column::TaskId.eq(task_id.into_uuid()))
            .order_by_asc(methodology_task_finding::Column::UpdatedAt)
            .order_by_asc(methodology_task_finding::Column::FindingId)
            .all(self.conn())
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| FindingId::from_uuid(row.finding_id))
            .collect())
    }

    /// Read finding ids projected for many `(spec, task)` pairs.
    ///
    /// Missing task ids are omitted from the returned map.
    ///
    /// # Errors
    /// Returns a store/database error on query failures.
    pub async fn load_methodology_finding_ids_for_tasks_projection(
        &self,
        spec_id: SpecId,
        task_ids: &[TaskId],
    ) -> StoreResult<std::collections::HashMap<TaskId, Vec<FindingId>>> {
        if task_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let mut out = std::collections::HashMap::new();
        for chunk in task_ids.chunks(SQLITE_CHUNK_SIZE) {
            let task_ids: Vec<uuid::Uuid> =
                chunk.iter().map(|task_id| task_id.into_uuid()).collect();
            let rows = methodology_task_finding::Entity::find()
                .filter(methodology_task_finding::Column::SpecId.eq(spec_id.into_uuid()))
                .filter(methodology_task_finding::Column::TaskId.is_in(task_ids))
                .order_by_asc(methodology_task_finding::Column::UpdatedAt)
                .order_by_asc(methodology_task_finding::Column::FindingId)
                .all(self.conn())
                .await?;
            for row in rows {
                out.entry(TaskId::from_uuid(row.task_id))
                    .or_insert_with(Vec::new)
                    .push(FindingId::from_uuid(row.finding_id));
            }
        }
        Ok(out)
    }
}

pub(crate) async fn upsert_task_finding_projection_txn(
    txn: &DatabaseTransaction,
    event: &MethodologyEvent,
) -> StoreResult<()> {
    match event {
        MethodologyEvent::FindingAdded(e) => {
            if let Some(task_id) = e.finding.attached_task {
                upsert_task_finding_projection(txn, e.finding.spec_id, task_id, e.finding.id)
                    .await?;
            }
        }
        MethodologyEvent::AdherenceFindingAdded(e) => {
            if let Some(task_id) = e.finding.attached_task {
                upsert_task_finding_projection(txn, e.finding.spec_id, task_id, e.finding.id)
                    .await?;
            }
        }
        _ => {}
    }
    Ok(())
}

async fn upsert_task_finding_projection<C: sea_orm::ConnectionTrait>(
    conn: &C,
    spec_id: SpecId,
    task_id: TaskId,
    finding_id: FindingId,
) -> StoreResult<()> {
    let now = Utc::now();
    let task_id = task_id.into_uuid();
    let finding_id = finding_id.into_uuid();
    let spec_id = spec_id.into_uuid();
    let row = methodology_task_finding::Entity::find()
        .filter(methodology_task_finding::Column::TaskId.eq(task_id))
        .filter(methodology_task_finding::Column::FindingId.eq(finding_id))
        .one(conn)
        .await?;
    match row {
        Some(model) => {
            let mut active = methodology_task_finding::ActiveModel::from(model);
            active.spec_id = Set(spec_id);
            active.updated_at = Set(now);
            active.update(conn).await?;
        }
        None => {
            methodology_task_finding::Entity::insert(methodology_task_finding::ActiveModel {
                task_id: Set(task_id),
                finding_id: Set(finding_id),
                spec_id: Set(spec_id),
                updated_at: Set(now),
            })
            .exec(conn)
            .await?;
        }
    }
    Ok(())
}
