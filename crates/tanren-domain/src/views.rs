//! Read-side projection types — what queries return.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::actor::ActorContext;
use crate::events::EventEnvelope;
use crate::ids::{DispatchId, StepId};
use crate::payloads::{DispatchSnapshot, StepPayload, StepResult};
use crate::status::{
    DispatchMode, DispatchStatus, Lane, Outcome, StepReadyState, StepStatus, StepType,
};

/// Read projection of a dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchView {
    pub dispatch_id: DispatchId,
    pub mode: DispatchMode,
    pub status: DispatchStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    pub lane: Lane,
    pub dispatch: Box<DispatchSnapshot>,
    /// Full actor attribution for policy / audit.
    pub actor: ActorContext,
    /// Current revision of the dispatch graph (incremented on replans).
    pub graph_revision: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Read projection of a step within a dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepView {
    pub step_id: StepId,
    pub dispatch_id: DispatchId,
    pub step_type: StepType,
    pub step_sequence: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<Lane>,
    pub status: StepStatus,
    /// Scheduler readiness — distinct from `status`. A step may be
    /// `Pending` while still `Blocked` on graph dependencies.
    pub ready_state: StepReadyState,
    /// Ordered step IDs this step depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<StepId>,
    /// Graph revision this step belongs to.
    pub graph_revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<StepPayload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<StepResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub retry_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Paginated query result for events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventQueryResult {
    pub events: Vec<EventEnvelope>,
    pub total_count: u64,
    pub has_more: bool,
}
