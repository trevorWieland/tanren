use tanren_domain::SpecId;
use tanren_domain::TaskId;
use tanren_domain::entity::EntityRef;
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_store::{EventFilter, EventStore};

use super::MethodologyService;
use super::errors::{MethodologyError, MethodologyResult};

impl MethodologyService {
    /// Resolve a task id to its spec id through the projection table,
    /// with indexed targeted recovery/backfill on projection misses.
    pub(crate) async fn resolve_spec_for_task(&self, task_id: TaskId) -> MethodologyResult<SpecId> {
        if let Some(spec_id) = self
            .store()
            .load_methodology_task_spec_projection(task_id)
            .await?
        {
            return Ok(spec_id);
        }
        tracing::warn!(task_id = %task_id, "task->spec projection miss; attempting targeted recovery");

        if let Some(status) = self
            .store()
            .load_methodology_task_status_projection_by_task_id(task_id)
            .await?
        {
            self.store()
                .upsert_methodology_task_spec_projection(task_id, status.spec_id)
                .await?;
            return Ok(status.spec_id);
        }

        let recovered = self
            .first_methodology_event_for_entity(EntityRef::Task(task_id))
            .await?;
        if let Some(MethodologyEvent::TaskCreated(e)) = recovered {
            self.store()
                .upsert_methodology_task_spec_projection(task_id, e.task.spec_id)
                .await?;
            return Ok(e.task.spec_id);
        }
        Err(MethodologyError::NotFound {
            resource: "task".into(),
            key: task_id.to_string(),
        })
    }

    pub(crate) async fn first_methodology_event_for_entity(
        &self,
        entity_ref: EntityRef,
    ) -> MethodologyResult<Option<MethodologyEvent>> {
        let filter = EventFilter {
            entity_ref: Some(entity_ref),
            event_type: Some("methodology".into()),
            limit: 1,
            ..EventFilter::new()
        };
        let page = EventStore::query_events(self.store(), &filter).await?;
        let Some(first) = page.events.into_iter().next() else {
            return Ok(None);
        };
        let DomainEvent::Methodology { event } = first.payload else {
            return Ok(None);
        };
        Ok(Some(event))
    }

    /// Emit a pre-built methodology event. Transport crates use this to
    /// compose higher-level workflows (e.g. `tanren session exit`
    /// emitting one `UnauthorizedArtifactEdit` per reverted file).
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn emit_event(
        &self,
        phase: &PhaseId,
        event: MethodologyEvent,
    ) -> MethodologyResult<()> {
        self.emit(phase, event).await.map(|_| ())
    }

    #[doc(hidden)]
    #[must_use]
    pub fn store(&self) -> &tanren_store::Store {
        self.store.as_ref()
    }
}
