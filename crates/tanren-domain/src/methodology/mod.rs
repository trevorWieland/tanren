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
//!
//! The 15 non-negotiables in
//! `docs/rewrite/tasks/LANE-0.5-BRIEF.md` govern design decisions here.

pub mod capability;
pub mod events;
pub mod evidence;
pub mod finding;
pub mod issue;
pub mod phase_outcome;
pub mod pillar;
pub mod rubric;
pub mod signpost;
pub mod spec;
pub mod standard;
pub mod task;

pub use events::{
    AdherenceFindingAdded, EvidenceSchemaError, FindingAdded, IssueCreated, MethodologyEvent,
    NonNegotiableComplianceRecorded, PhaseOutcomeReported, RubricScoreRecorded, SignpostAdded,
    SignpostStatusUpdated, SpecDefined, TaskAbandoned, TaskCompleted, TaskCreated,
    TaskGuardSatisfied, TaskImplemented, TaskRevised, TaskStarted, UnauthorizedArtifactEdit,
    fold_task_status,
};

pub use capability::{CapabilityScope, ToolCapability, default_scope_for_phase};
pub use finding::{Finding, FindingSeverity, FindingSource, StandardRef};
pub use issue::{Issue, IssuePriority, IssueProvider, IssueRef};
pub use phase_outcome::{BlockedReason, ErrorReason, PhaseOutcome};
pub use pillar::{ApplicableAt, Pillar, PillarId, PillarScope, PillarScore, builtin_pillars};
pub use rubric::{ComplianceStatus, NonNegotiableCompliance, RubricScore};
pub use signpost::{Signpost, SignpostStatus};
pub use spec::{
    ConnectionKind, DemoConnection, DemoEnvironment, Spec, SpecDependencies, SymbolKind,
    TouchedSymbol,
};
pub use standard::{Standard, StandardImportance};
pub use task::{AcceptanceCriterion, RequiredGuard, Task, TaskGuardFlags, TaskOrigin, TaskStatus};
