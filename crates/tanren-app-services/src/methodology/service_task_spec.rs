use tanren_domain::SpecId;
use tanren_domain::TaskId;
use tanren_domain::entity::EntityRef;
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_store::{EventFilter, EventStore};

use super::MethodologyService;
use super::errors::{MethodologyError, MethodologyResult};

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;

impl MethodologyService {
    /// Resolve a task id to its spec id through the projection table,
    /// with event-log scan fallback for migration backfill.
    pub(crate) async fn resolve_spec_for_task(&self, task_id: TaskId) -> MethodologyResult<SpecId> {
        if let Some(spec_id) = self
            .store()
            .load_methodology_task_spec_projection(task_id)
            .await?
        {
            return Ok(spec_id);
        }
        tracing::warn!(
            task_id = %task_id,
            "task->spec projection miss; falling back to methodology event scan"
        );
        let mut cursor = None;
        loop {
            let filter = EventFilter {
                entity_ref: Some(EntityRef::Task(task_id)),
                event_type: Some("methodology".into()),
                limit: METHODOLOGY_PAGE_SIZE,
                cursor,
                ..EventFilter::default()
            };
            let page = EventStore::query_events(self.store(), &filter).await?;
            for env in page.events {
                if let DomainEvent::Methodology { event } = env.payload
                    && let MethodologyEvent::TaskCreated(e) = &event
                {
                    self.store()
                        .upsert_methodology_task_spec_projection(task_id, e.task.spec_id)
                        .await?;
                    return Ok(e.task.spec_id);
                }
            }
            if !page.has_more {
                break;
            }
            cursor = page.next_cursor;
        }
        Err(MethodologyError::NotFound {
            resource: "task".into(),
            key: task_id.to_string(),
        })
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
