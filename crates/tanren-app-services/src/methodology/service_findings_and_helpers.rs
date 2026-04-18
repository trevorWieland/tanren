use std::future::Future;

use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tanren_domain::SpecId;
use tanren_domain::entity::EntityRef;
use tanren_domain::events::DomainEvent;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{FindingAdded, MethodologyEvent};
use tanren_domain::methodology::finding::Finding;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::{FindingId, TaskId};
use tanren_store::{EventFilter, EventStore};

use tanren_contract::methodology::{AddFindingParams, AddFindingResponse, SchemaVersion};

use super::MethodologyService;
use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;
const REQUEST_HASH_ALGO_SHA256_CANONICAL_JSON_V1: &str = "sha256-canonical-json-v1";
const REQUEST_HASH_ALGO_LEGACY_DEFAULT_HASHER_V0: &str = "default-hasher-json-v0";

impl MethodologyService {
    // -- §3.2 Findings --------------------------------------------------------

    /// `add_finding` — emit [`MethodologyEvent::FindingAdded`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn add_finding(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
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
        let canonical_payload =
            canonical_json(payload).map_err(|e| MethodologyError::Internal(e.to_string()))?;
        let request_hash = sha256_hex(canonical_payload.as_bytes());
        let legacy_request_hash = legacy_request_hash(&payload_json);
        let derived_key = explicit_key.unwrap_or_else(|| format!("payload:{request_hash}"));
        let scope_key = spec_id.to_string();

        let resolve_existing =
            |existing: tanren_store::methodology::MethodologyIdempotencyEntry| {
                let hash_matches = match existing.request_hash_algo.as_str() {
                    REQUEST_HASH_ALGO_SHA256_CANONICAL_JSON_V1 => {
                        existing.request_hash == request_hash
                    }
                    REQUEST_HASH_ALGO_LEGACY_DEFAULT_HASHER_V0 => {
                        existing.request_hash == legacy_request_hash
                    }
                    _ => {
                        existing.request_hash == request_hash
                            || existing.request_hash == legacy_request_hash
                    }
                };
                if !hash_matches {
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
                    request_hash_algo: REQUEST_HASH_ALGO_SHA256_CANONICAL_JSON_V1.into(),
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

fn sha256_hex(input: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(input);
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        write!(&mut out, "{b:02x}").expect("writing to string must not fail");
    }
    out
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let raw = serde_json::to_value(value)?;
    let canonical = canonicalize_value(raw);
    serde_json::to_string(&canonical)
}

fn canonicalize_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(canonicalize_value)
                .collect::<Vec<_>>(),
        ),
        serde_json::Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(value) = map.get(&key) {
                    sorted.insert(key, canonicalize_value(value.clone()));
                }
            }
            serde_json::Value::Object(sorted)
        }
        other => other,
    }
}

fn legacy_request_hash(payload_json: &str) -> String {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    payload_json.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tanren_contract::methodology::{AddFindingParams, AddFindingResponse, SchemaVersion};
    use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
    use tanren_domain::methodology::finding::{FindingSeverity, FindingSource};
    use tanren_domain::methodology::phase_id::PhaseId;
    use tanren_domain::{EntityKind, NonEmptyString, SpecId};
    use tanren_store::EventFilter;
    use tanren_store::methodology::InsertMethodologyIdempotencyParams;
    use tanren_store::{EventStore, Store};

    use crate::methodology::service::{MethodologyService, PhaseEventsRuntime};

    use super::{REQUEST_HASH_ALGO_LEGACY_DEFAULT_HASHER_V0, legacy_request_hash};

    #[tokio::test]
    async fn idempotency_accepts_legacy_default_hasher_rows() {
        let store = Arc::new(
            Store::open_and_migrate("sqlite::memory:?cache=shared")
                .await
                .expect("open"),
        );
        let runtime = PhaseEventsRuntime {
            spec_folder: std::env::temp_dir().join(format!(
                "tanren-methodology-idempotency-{}",
                uuid::Uuid::now_v7()
            )),
            agent_session_id: "test-session".into(),
        };
        let service =
            MethodologyService::with_runtime(store.clone(), vec![], Some(runtime), vec![]);
        let scope = CapabilityScope::from_iter_caps([ToolCapability::FindingAdd]);
        let phase = PhaseId::try_new("audit-task").expect("phase");
        let spec_id = SpecId::new();
        let params = AddFindingParams {
            schema_version: SchemaVersion::current(),
            spec_id,
            severity: FindingSeverity::FixNow,
            title: "legacy".into(),
            description: "legacy hash row".into(),
            affected_files: vec!["src/lib.rs".into()],
            line_numbers: vec![1],
            source: FindingSource::Audit {
                phase: NonEmptyString::try_new("audit-task").expect("phase"),
                pillar: None,
            },
            attached_task: None,
            idempotency_key: Some("legacy-key".into()),
        };
        let payload_json = serde_json::to_string(&params).expect("payload json");
        let legacy_hash = legacy_request_hash(&payload_json);
        let replay_response = AddFindingResponse {
            schema_version: SchemaVersion::current(),
            finding_id: tanren_domain::FindingId::new(),
        };
        store
            .insert_methodology_idempotency_reservation(InsertMethodologyIdempotencyParams {
                tool: "add_finding".into(),
                scope_key: spec_id.to_string(),
                idempotency_key: "legacy-key".into(),
                request_hash: legacy_hash,
                request_hash_algo: REQUEST_HASH_ALGO_LEGACY_DEFAULT_HASHER_V0.into(),
            })
            .await
            .expect("reserve");
        store
            .finalize_methodology_idempotency(
                "add_finding",
                &spec_id.to_string(),
                "legacy-key",
                serde_json::to_string(&replay_response).expect("response json"),
                None,
            )
            .await
            .expect("finalize");

        let returned = service
            .add_finding(&scope, &phase, params)
            .await
            .expect("replayed");
        assert_eq!(returned.finding_id, replay_response.finding_id);

        let events = store
            .query_events(&EventFilter {
                entity_kind: Some(EntityKind::Finding),
                spec_id: Some(spec_id),
                limit: 100,
                ..EventFilter::new()
            })
            .await
            .expect("query events");
        assert_eq!(
            events.events.len(),
            0,
            "replaying an existing idempotency row must not append duplicate events"
        );
    }
}
