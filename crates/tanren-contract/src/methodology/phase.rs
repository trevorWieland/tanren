//! Wire contract for phase-lifecycle tools (§3.6).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::phase_outcome::PhaseOutcome;

use super::SchemaVersion;

/// `report_phase_outcome` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReportPhaseOutcomeParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub phase: PhaseId,
    pub agent_session_id: String,
    pub outcome: PhaseOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `escalate_to_blocker` params. Capability-scoped to `investigate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EscalateToBlockerParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub reason: String,
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `post_reply_directive` params. Capability-scoped to `handle-feedback`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PostReplyDirectiveParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub thread_ref: String,
    pub body: String,
    pub disposition: FeedbackDisposition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// Directive the orchestrator will enact on the feedback thread.
///
/// This is a re-alias of [`tanren_domain::methodology::phase_outcome::ReplyDisposition`]
/// so the wire shape and the event-payload shape stay byte-identical.
pub type FeedbackDisposition = tanren_domain::methodology::phase_outcome::ReplyDisposition;
