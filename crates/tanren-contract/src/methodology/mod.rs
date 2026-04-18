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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

/// Semver-style version advertised alongside every tool in the
/// catalog. `list_tools` includes this value so a client can gate
/// behaviour on a minimum acceptable version.
///
/// Bumping rules:
/// - adding a new optional field or a new variant → bump the patch
///   or minor component (`1.0.0` → `1.1.0`).
/// - removing a field, renaming a variant, or altering semantics →
///   bump the major component **and** the namespace suffix
///   (`tanren.methodology.v1` → `tanren.methodology.v2`).
pub const METHODOLOGY_SCHEMA_VERSION: &str = "1.0.0";

/// Required payload-level schema version carried by every methodology
/// request/response body. This complements MCP `_meta` versioning and
/// lets non-MCP transports validate payload compatibility explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SchemaVersion(String);

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::current()
    }
}

impl SchemaVersion {
    /// Construct the current required schema version value.
    #[must_use]
    pub fn current() -> Self {
        Self(METHODOLOGY_SCHEMA_VERSION.to_owned())
    }

    /// Access as `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        if raw == METHODOLOGY_SCHEMA_VERSION {
            Ok(Self(raw))
        } else {
            Err(serde::de::Error::custom(format!(
                "unsupported schema_version `{raw}` (expected `{METHODOLOGY_SCHEMA_VERSION}`)"
            )))
        }
    }
}

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
pub use standard::{ListRelevantStandardsParams, RelevantStandard};
pub use task::{
    AbandonTaskParams, CompleteTaskParams, CreateTaskParams, CreateTaskResponse, ListTasksParams,
    MarkTaskGuardSatisfiedParams, ReviseTaskParams, StartTaskParams,
};
