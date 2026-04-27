//! Methodology subsystem — typed state machine for the spec/task
//! lifecycle, rubric scoring, standards adherence, signposts, and
//! backlog issues.
//!
//! All types in this module are pure domain values: no I/O, no async,
//! no workspace dependencies beyond `tanren-domain` itself. Service-
//! level orchestration (tool call entry points, event emission,
//! enforcement) lives in `tanren-app-services::methodology`.
//!
//! Architecture references:
//! - `docs/architecture/orchestration-flow.md` — task lifecycle
//! - `docs/architecture/agent-tool-surface.md` — tool catalog + scopes
//! - `docs/architecture/audit-rubric.md` — pillar definitions
//! - `docs/architecture/adherence.md` — standard-filter algorithm
//! - `docs/architecture/evidence-schemas.md` — evidence frontmatter
//! - `docs/architecture/install-targets.md` — variable taxonomy

pub mod capability;
pub mod check;
pub mod event_tool;
pub mod events;
pub mod evidence;
pub mod finding;
pub mod frontmatter_patch;
pub mod investigation;
pub mod issue;
pub mod lifecycle_events;
pub mod phase_id;
pub mod phase_outcome;
pub mod pillar;
pub mod rubric;
pub mod signpost;
pub mod spec;
pub mod standard;
pub mod task;
pub mod tool_catalog;
pub mod validation;

pub use events::{
    EvidenceSchemaError, IssueCreated, MethodologyEvent, NonNegotiableComplianceRecorded,
    PhaseOutcomeReported, RubricScoreRecorded, SignpostAdded, SignpostStatusUpdated, SpecDefined,
    TaskAbandoned, TaskAdherent, TaskAudited, TaskCompleted, TaskCreated, TaskGateChecked,
    TaskGuardsReset, TaskImplemented, TaskRevised, TaskStarted, TaskXChecked,
    UnauthorizedArtifactEdit, fold_task_status,
};
pub use lifecycle_events::{
    AdherenceFindingAdded, CheckFailureRecorded, CheckResultRecorded, CheckRunStarted,
    FindingAdded, FindingDeferred, FindingReopened, FindingResolved, FindingStillOpen,
    FindingSuperseded, InvestigationAttemptRecorded, RootCauseLinkedToFinding,
};

pub use capability::{
    CapabilityScope, PhaseCapabilityBinding, ToolCapability, default_phase_capability_bindings,
    default_scope_for_phase,
};
pub use check::{CheckKind, CheckResult, CheckRun, CheckScope, CheckStatus};
pub use event_tool::{
    PhaseEventOriginKind, allowed_tools_for_event, canonical_tool_for_event,
    is_tool_allowed_for_event,
};
pub use finding::{
    AdherenceSeverity, Finding, FindingLifecycleEvidence, FindingSeverity, FindingSource,
    FindingStatus, FindingView, StandardRef,
};
pub use investigation::{InvestigationAttempt, InvestigationRootCause, InvestigationSourceCheck};
pub use issue::{Issue, IssuePriority, IssueProvider, IssueRef};
pub use phase_id::{KnownPhase, PhaseId};
pub use phase_outcome::{BlockedReason, ErrorReason, PhaseOutcome};
pub use pillar::{ApplicableAt, Pillar, PillarId, PillarScope, PillarScore, builtin_pillars};
pub use rubric::{ComplianceStatus, NonNegotiableCompliance, RubricScore};
pub use signpost::{Signpost, SignpostStatus};
pub use spec::{
    ConnectionKind, DemoConnection, DemoEnvironment, Spec, SpecDependencies, SymbolKind,
    TouchedSymbol,
};
pub use standard::{Standard, StandardImportance};
pub use task::{
    AcceptanceCriterion, ExplicitUserDiscardProvenance, RequiredGuard, Task,
    TaskAbandonDisposition, TaskGuardFlags, TaskOrigin, TaskStatus,
};
pub use tool_catalog::{
    ToolDescriptor, ToolId, all_tool_descriptors, descriptor, descriptor_by_name,
};
pub use validation::{
    ValidationIssue, validate_finding_attached_task_spec, validate_finding_line_numbers,
    validate_task_abandon_semantics,
};
