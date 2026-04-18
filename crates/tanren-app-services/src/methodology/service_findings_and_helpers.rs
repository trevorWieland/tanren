use std::future::Future;
use std::hash::{Hash as _, Hasher as _};

use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use tanren_domain::SpecId;
use tanren_domain::entity::EntityRef;
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{FindingAdded, MethodologyEvent};
use tanren_domain::methodology::finding::Finding;
use tanren_domain::{FindingId, TaskId};
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
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "add_finding",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let title = require_non_empty("/title", &params.title, Some(200))?;
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
            },
        )
        .await
    }

    // -- Shared helpers -------------------------------------------------------

    pub(crate) async fn run_idempotent_mutation<R, P, F, Fut>(
        &self,
        tool: &str,
        spec_id: SpecId,
        explicit_key: Option<String>,
        payload: &P,
        op: F,
    ) -> MethodologyResult<R>
    where
        R: Serialize + DeserializeOwned,
        P: Serialize,
        F: FnOnce() -> Fut,
        Fut: Future<Output = MethodologyResult<R>>,
    {
        let payload_json = serde_json::to_string(payload)
            .map_err(|e| MethodologyError::Internal(e.to_string()))?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        payload_json.hash(&mut hasher);
        let request_hash = format!("{:016x}", hasher.finish());
        let derived_key = explicit_key.unwrap_or_else(|| format!("payload:{request_hash}"));
        let scope_key = spec_id.to_string();

        let resolve_existing =
            |existing: tanren_store::methodology::MethodologyIdempotencyEntry| {
                if existing.request_hash != request_hash {
                    return Err(MethodologyError::Conflict {
                        resource: tool.to_owned(),
                        reason: format!(
                            "idempotency key `{}` reused with different payload hash",
                            existing.idempotency_key
                        ),
                    });
                }
                let Some(response_json) = existing.response_json else {
                    return Err(MethodologyError::Conflict {
                        resource: tool.to_owned(),
                        reason: format!(
                            "idempotency key `{}` is reserved by an unfinished prior attempt",
                            existing.idempotency_key
                        ),
                    });
                };
                serde_json::from_str::<R>(&response_json).map_err(|e| {
                    MethodologyError::Internal(format!("idempotency replay decode: {e}"))
                })
            };

        if let Some(existing) = self
            .store
            .get_methodology_idempotency(tool, &scope_key, &derived_key)
            .await?
        {
            return resolve_existing(existing);
        }

        let inserted = self
            .store
            .insert_methodology_idempotency_reservation(
                tanren_store::methodology::InsertMethodologyIdempotencyParams {
                    tool: tool.to_owned(),
                    scope_key: scope_key.clone(),
                    idempotency_key: derived_key.clone(),
                    request_hash: request_hash.clone(),
                },
            )
            .await?;
        if !inserted
            && let Some(existing) = self
                .store
                .get_methodology_idempotency(tool, &scope_key, &derived_key)
                .await?
        {
            return resolve_existing(existing);
        }

        match op().await {
            Ok(response) => {
                let response_json = serde_json::to_string(&response)
                    .map_err(|e| MethodologyError::Internal(e.to_string()))?;
                self.store
                    .finalize_methodology_idempotency(
                        tool,
                        &scope_key,
                        &derived_key,
                        response_json,
                        None,
                    )
                    .await?;
                Ok(response)
            }
            Err(err) => {
                let _ = self
                    .store
                    .delete_methodology_idempotency(tool, &scope_key, &derived_key)
                    .await;
                Err(err)
            }
        }
    }

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
