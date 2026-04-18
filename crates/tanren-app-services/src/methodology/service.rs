//! `MethodologyService` — the concrete service both CLI and MCP
//! transports call. One method per tool in the catalog. Each method:
//!
//! 1. Enforces the caller's capability scope via
//!    [`super::capabilities::enforce`].
//! 2. Validates inputs against the domain invariants.
//! 3. Emits exactly one `DomainEvent::Methodology { .. }` via
//!    `tanren_store::Store::append_methodology_event`.
//! 4. Returns the typed response per the contract.
//!
//! Tool methods are small (≤ 100 lines) and uniform so tool-catalog
//! growth stays boilerplate-minimal.

use std::sync::Arc;

use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    FindingAdded, MethodologyEvent, TaskAbandoned as EvTaskAbandoned, TaskCreated as EvTaskCreated,
    TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::finding::Finding;
use tanren_domain::methodology::task::{
    LegalTransition, RequiredGuard, Task, TaskStatus, TaskTransitionKind,
};
use tanren_domain::{EventId, FindingId, SpecId, TaskId};
use tanren_store::Store;

use tanren_contract::methodology::{
    AbandonTaskParams, AddFindingParams, AddFindingResponse, CompleteTaskParams, CreateTaskParams,
    CreateTaskResponse, StartTaskParams,
};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};

/// Shared methodology service.
///
/// Both `tanren-cli` and `tanren-mcp` take `Arc<MethodologyService>`.
/// The service is transport-agnostic; transports only supply the
/// caller's [`CapabilityScope`] and phase name.
#[derive(Debug, Clone)]
pub struct MethodologyService {
    store: Arc<Store>,
}

impl MethodologyService {
    /// Construct a service over a shared store handle.
    #[must_use]
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    fn new_envelope(payload: MethodologyEvent) -> EventEnvelope {
        EventEnvelope {
            schema_version: tanren_domain::SCHEMA_VERSION,
            event_id: EventId::new(),
            timestamp: Utc::now(),
            entity_ref: payload.entity_root(),
            payload: DomainEvent::Methodology { event: payload },
        }
    }

    async fn emit(&self, event: MethodologyEvent) -> MethodologyResult<EventEnvelope> {
        let envelope = Self::new_envelope(event);
        self.store.append_methodology_event(&envelope).await?;
        Ok(envelope)
    }

    // -- §3.1 Core task operations -------------------------------------------

    /// `create_task` — emit [`MethodologyEvent::TaskCreated`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn create_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: CreateTaskParams,
    ) -> MethodologyResult<CreateTaskResponse> {
        enforce(scope, ToolCapability::TaskCreate, phase)?;
        let title = require_non_empty("/title", &params.title, Some(160))?;
        let now = Utc::now();
        let task = Task {
            id: TaskId::new(),
            spec_id: params.spec_id,
            title,
            description: params.description,
            acceptance_criteria: params.acceptance_criteria,
            origin: params.origin.clone(),
            status: TaskStatus::Pending,
            depends_on: params.depends_on,
            parent_task_id: params.parent_task_id,
            created_at: now,
            updated_at: now,
        };
        let task_id = task.id;
        self.emit(MethodologyEvent::TaskCreated(EvTaskCreated {
            task: Box::new(task),
            origin: params.origin,
        }))
        .await?;
        Ok(CreateTaskResponse { task_id })
    }

    /// `start_task` — emit [`MethodologyEvent::TaskStarted`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn start_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: StartTaskParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::TaskStart, phase)?;
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        match self
            .check_transition(spec_id, params.task_id, TaskTransitionKind::Start)
            .await?
        {
            LegalTransition::Transition => {
                self.emit(MethodologyEvent::TaskStarted(TaskStarted {
                    task_id: params.task_id,
                    spec_id,
                }))
                .await?;
            }
            LegalTransition::Idempotent => {}
        }
        Ok(())
    }

    /// `complete_task` — emit [`MethodologyEvent::TaskImplemented`].
    /// (The `TaskCompleted` transition to terminal state fires later,
    /// once all required guards have arrived.)
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn complete_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: CompleteTaskParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::TaskComplete, phase)?;
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        match self
            .check_transition(spec_id, params.task_id, TaskTransitionKind::Implement)
            .await?
        {
            LegalTransition::Transition => {
                self.emit(MethodologyEvent::TaskImplemented(TaskImplemented {
                    task_id: params.task_id,
                    spec_id,
                    evidence_refs: params.evidence_refs,
                }))
                .await?;
            }
            LegalTransition::Idempotent => {}
        }
        Ok(())
    }

    /// `abandon_task` — emit [`MethodologyEvent::TaskAbandoned`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn abandon_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: AbandonTaskParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::TaskAbandon, phase)?;
        // F8: abandon requires either a non-empty `reason` *and* at
        // least one replacement task, or an explicit user-discard note.
        // The contract accepts both as a single shape; replacements
        // non-empty implies "replaced" semantics, empty implies
        // "user-discarded" and the reason must be substantive.
        let reason = require_non_empty("/reason", &params.reason, Some(500))?;
        if params.replacements.is_empty() && reason.as_str().trim().len() < 8 {
            return Err(MethodologyError::FieldValidation {
                field_path: "/replacements".into(),
                expected: "non-empty replacements[] OR /reason describing the discard"
                    .into(),
                actual: format!(
                    "replacements=[], reason={} chars",
                    reason.as_str().trim().len()
                ),
                remediation:
                    "either supply at least one replacement task id, or describe the discard in /reason (≥ 8 chars)".into(),
            });
        }
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        // Legal from any non-terminal state; idempotent if already
        // abandoned with the same replacements (content hash elsewhere).
        match self
            .check_transition(spec_id, params.task_id, TaskTransitionKind::Abandon)
            .await?
        {
            LegalTransition::Transition => {
                self.emit(MethodologyEvent::TaskAbandoned(EvTaskAbandoned {
                    task_id: params.task_id,
                    spec_id,
                    reason,
                    replacements: params.replacements,
                }))
                .await?;
            }
            LegalTransition::Idempotent => {}
        }
        Ok(())
    }

    /// Fold the current status of `task_id` from the event log and
    /// consult [`TaskStatus::legal_next`]. Returns a typed
    /// [`MethodologyError::IllegalTaskTransition`] on rejection.
    async fn check_transition(
        &self,
        spec_id: SpecId,
        task_id: TaskId,
        kind: TaskTransitionKind,
    ) -> MethodologyResult<LegalTransition> {
        let required = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        let events = tanren_store::methodology::projections::load_methodology_events(
            self.store(),
            spec_id,
            u64::MAX,
        )
        .await?;
        let current =
            tanren_domain::methodology::events::fold_task_status(task_id, &required, events.iter())
                .unwrap_or(TaskStatus::Pending);
        current
            .legal_next(kind)
            .map_err(|e| MethodologyError::IllegalTaskTransition {
                task_id,
                from: e.from.to_owned(),
                attempted: e.attempted.to_owned(),
            })
    }

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
        self.emit(MethodologyEvent::FindingAdded(FindingAdded {
            finding: Box::new(finding),
        }))
        .await?;
        Ok(AddFindingResponse { finding_id: id })
    }

    // -- Shared helpers -------------------------------------------------------

    /// Resolve a task id to its spec id by scanning the event log for
    /// the corresponding `TaskCreated` event.
    ///
    /// O(events) per call. Acceptable at Lane 0.5 scale (spec event
    /// counts in the hundreds to low thousands); Phase 1+ may add a
    /// projection table indexed by task id if profiling warrants.
    pub(crate) async fn resolve_spec_for_task(&self, task_id: TaskId) -> MethodologyResult<SpecId> {
        let filter = tanren_store::EventFilter {
            event_type: Some("methodology".into()),
            limit: u64::MAX,
            ..tanren_store::EventFilter::default()
        };
        let page = tanren_store::EventStore::query_events(self.store.as_ref(), &filter).await?;
        for env in page.events {
            if let DomainEvent::Methodology { event } = env.payload
                && let MethodologyEvent::TaskCreated(e) = &event
                && e.task.id == task_id
            {
                return Ok(e.task.spec_id);
            }
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
    pub async fn emit_event(&self, event: MethodologyEvent) -> MethodologyResult<()> {
        self.emit(event).await.map(|_| ())
    }

    #[doc(hidden)]
    #[must_use]
    pub fn store(&self) -> &Store {
        self.store.as_ref()
    }
}
