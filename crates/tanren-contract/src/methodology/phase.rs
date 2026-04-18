//! Wire contract for phase-lifecycle tools (§3.6).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;
use tanren_domain::methodology::phase_outcome::PhaseOutcome;

/// `report_phase_outcome` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReportPhaseOutcomeParams {
    pub spec_id: SpecId,
    pub phase: String,
    pub agent_session_id: String,
    pub outcome: PhaseOutcome,
}

/// `escalate_to_blocker` params. Capability-scoped to `investigate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EscalateToBlockerParams {
    pub spec_id: SpecId,
    pub reason: String,
    pub options: Vec<String>,
}

/// `post_reply_directive` params. Capability-scoped to `handle-feedback`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PostReplyDirectiveParams {
    pub spec_id: SpecId,
    pub thread_ref: String,
    pub body: String,
    pub disposition: FeedbackDisposition,
}

/// Directive the orchestrator will enact on the feedback thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackDisposition {
    Ack,
    Defer,
    Dispute,
}
