use sea_orm::EntityTrait;
use tanren_domain::TaskId;

use crate::Store;
use crate::entity::methodology_task_status;
use crate::errors::StoreResult;

use super::task_status_projection::TaskStatusProjection;

impl Store {
    /// Read one task-status projection row by task id regardless of spec.
    ///
    /// # Errors
    /// Returns a store/database or conversion error on invalid projection rows.
    pub async fn load_methodology_task_status_projection_by_task_id(
        &self,
        task_id: TaskId,
    ) -> StoreResult<Option<TaskStatusProjection>> {
        let Some(row) = methodology_task_status::Entity::find_by_id(task_id.into_uuid())
            .one(self.conn())
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(TaskStatusProjection::try_from_model(&row)?))
    }
}
