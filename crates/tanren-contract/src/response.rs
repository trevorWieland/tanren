//! Outbound response types returned to transport interfaces.
//!
//! These types represent the canonical output shapes returned by all
//! transport interfaces. They map from domain views to wire-safe
//! representations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use tanren_domain::{
    Cli, DispatchMode, DispatchStatus, DispatchView, Lane, Outcome, Phase, StepView,
};

/// Response representing a single dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchResponse {
    pub dispatch_id: Uuid,
    pub status: DispatchStatus,
    pub mode: DispatchMode,
    pub lane: Lane,
    pub project: String,
    pub phase: Phase,
    pub cli: Cli,
    pub branch: String,
    pub spec_folder: String,
    pub workflow_id: String,
    pub environment_profile: String,
    pub timeout_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response containing a list of dispatches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchListResponse {
    pub dispatches: Vec<DispatchResponse>,
}

/// Response representing a single step within a dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResponse {
    pub step_id: Uuid,
    pub dispatch_id: Uuid,
    pub step_type: String,
    pub step_sequence: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    pub status: String,
    pub ready_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub retry_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DispatchView> for DispatchResponse {
    fn from(view: DispatchView) -> Self {
        Self {
            dispatch_id: view.dispatch_id.into_uuid(),
            status: view.status,
            mode: view.mode,
            lane: view.lane,
            project: view.dispatch.project.as_str().to_owned(),
            phase: view.dispatch.phase,
            cli: view.dispatch.cli,
            branch: view.dispatch.branch.as_str().to_owned(),
            spec_folder: view.dispatch.spec_folder.as_str().to_owned(),
            workflow_id: view.dispatch.workflow_id.as_str().to_owned(),
            environment_profile: view.dispatch.environment_profile.as_str().to_owned(),
            timeout_secs: view.dispatch.timeout.get(),
            outcome: view.outcome,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}

impl From<StepView> for StepResponse {
    fn from(view: StepView) -> Self {
        Self {
            step_id: view.step_id.into_uuid(),
            dispatch_id: view.dispatch_id.into_uuid(),
            step_type: view.step_type.to_string(),
            step_sequence: view.step_sequence,
            lane: view.lane.map(|l| l.to_string()),
            status: view.status.to_string(),
            ready_state: view.ready_state.to_string(),
            worker_id: view.worker_id,
            error: view.error,
            retry_count: view.retry_count,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}
