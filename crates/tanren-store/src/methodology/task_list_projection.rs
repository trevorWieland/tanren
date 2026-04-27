use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use tanren_domain::methodology::task::{Task, TaskStatus};
use tanren_domain::{SpecId, TaskId};

use crate::Store;
use crate::entity::methodology_task_status;
use crate::errors::{StoreError, StoreResult};

use super::task_status_projection::{
    TaskStatusProjection, encode_task_snapshot, parse_task_snapshot,
};

/// One row from the task-list projection read path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListProjectionRow {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    pub status: TaskStatus,
    pub task: Option<Task>,
}

impl Store {
    /// Read all task projection rows for one spec, ordered for stable list output.
    ///
    /// # Errors
    /// Returns a store/database or conversion error on invalid projection rows.
    pub async fn load_methodology_task_list_projection(
        &self,
        spec_id: SpecId,
    ) -> StoreResult<Vec<TaskListProjectionRow>> {
        let rows = methodology_task_status::Entity::find()
            .filter(methodology_task_status::Column::SpecId.eq(spec_id.into_uuid()))
            .order_by_asc(methodology_task_status::Column::CreatedAt)
            .order_by_asc(methodology_task_status::Column::TaskId)
            .all(self.conn())
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let projection = TaskStatusProjection::try_from_model(&row)?;
            let mut task = parse_task_snapshot(&row)?;
            if let Some(parsed_task) = task.as_mut() {
                parsed_task.status = projection.status.clone();
            }
            out.push(TaskListProjectionRow {
                task_id: projection.task_id,
                spec_id: projection.spec_id,
                status: projection.status,
                task,
            });
        }
        Ok(out)
    }

    /// Upsert one task projection row using a full task snapshot payload.
    ///
    /// # Errors
    /// Returns a store/database or conversion error on write failures.
    pub async fn upsert_methodology_task_projection_snapshot(
        &self,
        task: &Task,
    ) -> StoreResult<()> {
        self.upsert_methodology_task_status_projection(task.spec_id, task.id, &task.status)
            .await?;
        let Some(model) = methodology_task_status::Entity::find_by_id(task.id.into_uuid())
            .filter(methodology_task_status::Column::SpecId.eq(task.spec_id.into_uuid()))
            .one(self.conn())
            .await?
        else {
            return Err(StoreError::NotFound {
                entity_kind: tanren_domain::EntityKind::Task,
                id: task.id.to_string(),
            });
        };
        let mut active = methodology_task_status::ActiveModel::from(model);
        active.task_json = Set(Some(encode_task_snapshot(task)?));
        active.created_at = Set(Some(task.created_at));
        active.updated_at = Set(Utc::now());
        active.update(self.conn()).await?;
        Ok(())
    }
}
