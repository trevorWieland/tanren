use tanren_domain::SpecId;

use super::errors::MethodologyResult;
use super::service::{MethodologyService, ProjectionReconcileReport};

impl MethodologyService {
    /// Rebuild methodology lookup/list projections for one spec.
    ///
    /// This is an explicit repair path for legacy/corrupt projection
    /// states. It may scan a full spec event history and should be
    /// invoked proactively, not on read-hot paths.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn reconcile_methodology_projections_for_spec(
        &self,
        spec_id: SpecId,
    ) -> MethodologyResult<ProjectionReconcileReport> {
        let tasks = tanren_store::methodology::projections::tasks_for_spec(
            self.store(),
            spec_id,
            self.required_guards(),
        )
        .await?;
        let signposts =
            tanren_store::methodology::projections::signposts_for_spec(self.store(), spec_id)
                .await?;

        let mut task_spec_rows_repaired = 0_u64;
        let mut signpost_spec_rows_repaired = 0_u64;
        for task in &tasks {
            self.store()
                .upsert_methodology_task_projection_snapshot(task)
                .await?;
            self.store()
                .upsert_methodology_task_spec_projection(task.id, task.spec_id)
                .await?;
            task_spec_rows_repaired = task_spec_rows_repaired.saturating_add(1);
        }
        for signpost in &signposts {
            self.store()
                .upsert_methodology_signpost_spec_projection(signpost.id, signpost.spec_id)
                .await?;
            signpost_spec_rows_repaired = signpost_spec_rows_repaired.saturating_add(1);
        }

        Ok(ProjectionReconcileReport {
            tasks_rebuilt: tasks.len().try_into().unwrap_or(u64::MAX),
            task_spec_rows_repaired,
            signpost_spec_rows_repaired,
        })
    }
}
