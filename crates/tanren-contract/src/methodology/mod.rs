//! Wire contract for the methodology tool surface.
//!
//! Every request and response type below derives [`schemars::JsonSchema`]
//! so the MCP transport can publish authoritative schemas at
//! registration time and the CLI can produce matching argument parsers.
//!
//! # Versioning
//!
//! The stable schema-document namespace is
//! [`METHODOLOGY_SCHEMA_NAMESPACE`]. Adding a new optional field is a
//! minor-compatible change; renaming or removing a field is a major
//! bump (update the namespace). This is **independent** of
//! [`tanren_domain::SCHEMA_VERSION`], which governs the on-disk event
//! envelope.

pub mod demo;
pub mod finding;
pub mod issue;
pub mod phase;
pub mod rubric;
pub mod signpost;
pub mod spec;
pub mod standard;
pub mod task;

/// Stable JSON Schema namespace for the methodology tool surface.
pub const METHODOLOGY_SCHEMA_NAMESPACE: &str = "tanren.methodology.v1";

pub use demo::{AddDemoStepParams, AppendDemoResultParams, MarkDemoStepSkipParams};
pub use finding::{AddFindingParams, AddFindingResponse, RecordAdherenceFindingParams};
pub use issue::{CreateIssueParams, CreateIssueResponse};
pub use phase::{EscalateToBlockerParams, PostReplyDirectiveParams, ReportPhaseOutcomeParams};
pub use rubric::{RecordNonNegotiableComplianceParams, RecordRubricScoreParams};
pub use signpost::{AddSignpostParams, AddSignpostResponse, UpdateSignpostStatusParams};
pub use spec::{
    AddSpecAcceptanceCriterionParams, SetSpecBaseBranchParams, SetSpecDemoEnvironmentParams,
    SetSpecDependenciesParams, SetSpecNonNegotiablesParams, SetSpecTitleParams,
};
pub use standard::ListRelevantStandardsParams;
pub use task::{
    AbandonTaskParams, CompleteTaskParams, CreateTaskParams, CreateTaskResponse, ListTasksParams,
    ReviseTaskParams, StartTaskParams,
};
