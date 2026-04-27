//! Strongly-typed identifier newtypes wrapping [`uuid::Uuid`] v7.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generates a newtype ID wrapper around [`Uuid`].
///
/// Each generated type gets `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`,
/// `Hash`, `Serialize`, `Deserialize`, `JsonSchema` (transparent over
/// [`uuid::Uuid`] so the schema stays `"format": "uuid"`), transparent
/// serde, `Display` delegating to the inner UUID, `new()` generating
/// v7, and `from_uuid()`.
macro_rules! define_id {
    ($($(#[doc = $doc:expr])* $name:ident),+ $(,)?) => {
        $(
            $(#[doc = $doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
            #[serde(transparent)]
            #[schemars(transparent)]
            pub struct $name(Uuid);

            impl $name {
                /// Create a new time-ordered (v7) identifier.
                #[must_use]
                pub fn new() -> Self {
                    Self(Uuid::now_v7())
                }

                /// Wrap an existing [`Uuid`].
                #[must_use]
                pub const fn from_uuid(uuid: Uuid) -> Self {
                    Self(uuid)
                }

                /// Return the inner [`Uuid`].
                #[must_use]
                pub const fn into_uuid(self) -> Uuid {
                    self.0
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    self.0.fmt(f)
                }
            }
        )+
    };
}

define_id!(
    /// Identifies a dispatch (top-level orchestration unit).
    DispatchId,
    /// Identifies a step within a dispatch.
    StepId,
    /// Identifies an execution lease.
    LeaseId,
    /// Identifies a user.
    UserId,
    /// Identifies a team.
    TeamId,
    /// Identifies an organization (top-level multi-tenant boundary).
    OrgId,
    /// Identifies an API key.
    ApiKeyId,
    /// Identifies a project.
    ProjectId,
    /// Identifies a domain event.
    EventId,
    /// Identifies a methodology spec (top-level unit of planned work).
    SpecId,
    /// Identifies a methodology task within a spec.
    TaskId,
    /// Identifies a finding produced by audit / adherence / demo / investigation.
    FindingId,
    /// Identifies one generic methodology check run.
    CheckRunId,
    /// Identifies one durable investigation attempt.
    InvestigationAttemptId,
    /// Identifies one typed investigation root cause.
    RootCauseId,
    /// Identifies a signpost entry recorded during task implementation.
    SignpostId,
    /// Identifies a tracked backlog issue (provider-agnostic).
    IssueId,
);
