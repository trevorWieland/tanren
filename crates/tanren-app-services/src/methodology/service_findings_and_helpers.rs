use chrono::Utc;
use tanren_domain::SpecId;
use tanren_domain::entity::EntityRef;
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{FindingAdded, MethodologyEvent};
use tanren_domain::methodology::finding::Finding;
use tanren_domain::{EntityKind, FindingId, TaskId};
use tanren_store::{EventFilter, EventStore};

use tanren_contract::methodology::{AddFindingParams, AddFindingResponse, SchemaVersion};

use super::MethodologyService;
use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;

impl MethodologyService {
    // -- §3.2 Findings --------------------------------------------------------

    /// `add_finding` — emit [`MethodologyEvent::FindingAdded`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn add_finding(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: AddFindingParams,
    ) -> MethodologyResult<AddFindingResponse> {
        enforce(scope, ToolCapability::FindingAdd, phase)?;
        let title = require_non_empty("/title", &params.title, Some(200))?;
        if let Some(key) = params.idempotency_key.as_deref()
            && let Some(existing) = self
                .find_finding_added_by_idempotency(params.spec_id, key)
                .await?
        {
            let semantically_same = existing.severity == params.severity
                && existing.title == title
                && existing.description == params.description
                && existing.affected_files == params.affected_files
                && existing.line_numbers == params.line_numbers
                && existing.source == params.source
                && existing.attached_task == params.attached_task;
            if !semantically_same {
                return Err(MethodologyError::Conflict {
                    resource: "add_finding".into(),
                    reason: format!(
                        "idempotency_key `{key}` already used with different payload for finding {}",
                        existing.id
                    ),
                });
            }
            return Ok(AddFindingResponse {
                schema_version: SchemaVersion::current(),
                finding_id: existing.id,
            });
        }
        let finding = Finding {
            id: FindingId::new(),
            spec_id: params.spec_id,
            severity: params.severity,
            title,
            description: params.description,
            affected_files: params.affected_files,
            line_numbers: params.line_numbers,
            source: params.source,
            attached_task: params.attached_task,
            created_at: Utc::now(),
        };
        let id = finding.id;
        self.emit(
            phase,
            MethodologyEvent::FindingAdded(FindingAdded {
                finding: Box::new(finding),
                idempotency_key: params.idempotency_key,
            }),
        )
        .await?;
        Ok(AddFindingResponse {
            schema_version: SchemaVersion::current(),
            finding_id: id,
        })
    }

    // -- Shared helpers -------------------------------------------------------

    /// Resolve a task id to its spec id by querying task-root events.
    pub(crate) async fn resolve_spec_for_task(&self, task_id: TaskId) -> MethodologyResult<SpecId> {
        if let Ok(cache) = self.task_spec_cache.lock()
            && let Some(spec_id) = cache.get(&task_id)
        {
            return Ok(*spec_id);
        }
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
                    if let Ok(mut cache) = self.task_spec_cache.lock() {
                        cache.insert(task_id, e.task.spec_id);
                    }
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

    pub(crate) async fn find_task_created_by_idempotency(
        &self,
        spec_id: SpecId,
        idempotency_key: &str,
    ) -> MethodologyResult<Option<tanren_domain::methodology::task::Task>> {
        let events = tanren_store::methodology::projections::load_methodology_events_for_kind(
            self.store(),
            spec_id,
            METHODOLOGY_PAGE_SIZE,
            EntityKind::Task,
        )
        .await?;
        for event in events {
            if let MethodologyEvent::TaskCreated(e) = event
                && e.idempotency_key.as_deref() == Some(idempotency_key)
            {
                return Ok(Some(*e.task));
            }
        }
        Ok(None)
    }

    async fn find_finding_added_by_idempotency(
        &self,
        spec_id: SpecId,
        idempotency_key: &str,
    ) -> MethodologyResult<Option<Finding>> {
        let events = tanren_store::methodology::projections::load_methodology_events_for_kind(
            self.store(),
            spec_id,
            METHODOLOGY_PAGE_SIZE,
            EntityKind::Finding,
        )
        .await?;
        for event in events {
            if let MethodologyEvent::FindingAdded(e) = event
                && e.idempotency_key.as_deref() == Some(idempotency_key)
            {
                return Ok(Some(*e.finding));
            }
        }
        Ok(None)
    }

    /// Emit a pre-built methodology event. Transport crates use this to
    /// compose higher-level workflows (e.g. `tanren session exit`
    /// emitting one `UnauthorizedArtifactEdit` per reverted file).
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn emit_event(&self, phase: &str, event: MethodologyEvent) -> MethodologyResult<()> {
        self.emit(phase, event).await.map(|_| ())
    }

    #[doc(hidden)]
    #[must_use]
    pub fn store(&self) -> &tanren_store::Store {
        self.store.as_ref()
    }
}
