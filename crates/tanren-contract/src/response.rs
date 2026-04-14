//! Outbound response types returned to transport interfaces.
//!
//! These types represent the canonical output shapes returned by all
//! transport interfaces. They map from domain views to wire-safe
//! representations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use tanren_domain::{DispatchView, StepView};

use crate::enums::{
    AuthMode, Cli, DispatchMode, DispatchStatus, Lane, Outcome, Phase, StepReadyState, StepStatus,
    StepType,
};
use crate::request::DispatchCursorToken;

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
    pub auth_mode: AuthMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_env_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_secrets: Vec<String>,
    #[serde(default)]
    pub preserve_on_failure: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response containing a list of dispatches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchListResponse {
    pub dispatches: Vec<DispatchResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<DispatchCursorToken>,
}

/// Response representing a single step within a dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResponse {
    pub step_id: Uuid,
    pub dispatch_id: Uuid,
    pub step_type: StepType,
    pub step_sequence: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<Lane>,
    pub status: StepStatus,
    pub ready_state: StepReadyState,
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
            status: view.status.into(),
            mode: view.mode.into(),
            lane: view.lane.into(),
            project: view.dispatch.project.as_str().to_owned(),
            phase: view.dispatch.phase.into(),
            cli: view.dispatch.cli.into(),
            branch: view.dispatch.branch.as_str().to_owned(),
            spec_folder: view.dispatch.spec_folder.as_str().to_owned(),
            workflow_id: view.dispatch.workflow_id.as_str().to_owned(),
            environment_profile: view.dispatch.environment_profile.as_str().to_owned(),
            timeout_secs: view.dispatch.timeout.get(),
            auth_mode: view.dispatch.auth_mode.into(),
            gate_cmd: view.dispatch.gate_cmd,
            context: view.dispatch.context,
            model: view.dispatch.model,
            project_env_keys: view.dispatch.project_env.as_slice().to_vec(),
            required_secrets: view.dispatch.required_secrets,
            preserve_on_failure: view.dispatch.preserve_on_failure,
            outcome: view.outcome.map(Into::into),
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
            step_type: view.step_type.into(),
            step_sequence: view.step_sequence,
            lane: view.lane.map(Into::into),
            status: view.status.into(),
            ready_state: view.ready_state.into(),
            worker_id: view.worker_id,
            error: view.error,
            retry_count: view.retry_count,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}
