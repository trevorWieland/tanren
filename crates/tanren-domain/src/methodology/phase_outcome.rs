//! Typed phase outcomes.
//!
//! Every agentic phase concludes by calling `report_phase_outcome` exactly
//! once. The outcome is one of three variants per
//! `docs/architecture/agent-tool-surface.md` §3.6.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::validated::NonEmptyString;

/// Directive the orchestrator will enact on a feedback thread. The
/// wire contract in `tanren-contract::methodology::phase` mirrors this
/// enum verbatim (via a plain re-alias) so the domain owns the
/// canonical shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReplyDisposition {
    Ack,
    Defer,
    Dispute,
}

/// Typed result of one phase execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PhaseOutcome {
    /// Phase achieved its declared goal.
    Complete {
        summary: NonEmptyString,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_action_hint: Option<NonEmptyString>,
    },
    /// Phase reached a typed blocker; the orchestrator will follow the
    /// escalation ladder.
    Blocked {
        reason: BlockedReason,
        summary: NonEmptyString,
    },
    /// Phase hit an unexpected error. The orchestrator retries with a
    /// fresh session (up to the policy's retry cap).
    Error {
        reason: ErrorReason,
        summary: NonEmptyString,
    },
}

impl PhaseOutcome {
    /// Short `snake_case` tag for logging and display.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Complete { .. } => "complete",
            Self::Blocked { .. } => "blocked",
            Self::Error { .. } => "error",
        }
    }
}

/// Typed reason a phase is blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockedReason {
    /// Awaiting a human decision (e.g. resolve-blockers).
    AwaitingHumanInput { prompt: NonEmptyString },
    /// External dependency unavailable (e.g. API down).
    ExternalDependency {
        name: NonEmptyString,
        detail: String,
    },
    /// Investigate's loop cap reached; escalation required.
    InvestigationLoopCap { loop_index: u16 },
    /// Spec ambiguity prevents forward progress.
    SpecAmbiguity { detail: NonEmptyString },
    /// Generic, free-text blocker for cases the typed set doesn't cover.
    Other { detail: NonEmptyString },
}

/// Typed reason a phase errored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ErrorReason {
    /// Transient network / timeout / rate-limit failure.
    Transient { detail: NonEmptyString },
    /// Tool call rejected by the service layer.
    ToolError { detail: NonEmptyString },
    /// Agent produced no tool call before session end.
    NoProgress,
    /// Other, typed-but-not-otherwise-classified.
    Other { detail: NonEmptyString },
}
