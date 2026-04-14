//! Inbound request types for contract operations.
//!
//! These types represent the canonical input shapes consumed by all
//! transport interfaces (CLI, API, MCP, TUI). String fields for domain
//! enums allow the contract to stay stable across domain enum additions;
//! validation happens in the [`TryFrom`] conversion to domain commands.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tanren_domain::{AuthMode, Cli, DispatchMode, DispatchStatus, Lane, Phase};
use uuid::Uuid;

/// Request to create a new dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateDispatchRequest {
    // -- Actor attribution (required) --
    /// Organization UUID (v7).
    pub org_id: Uuid,
    /// User UUID (v7).
    pub user_id: Uuid,

    // -- Required dispatch fields --
    pub project: String,
    /// Phase of work.
    pub phase: Phase,
    /// CLI harness.
    pub cli: Cli,
    pub branch: String,
    pub spec_folder: String,
    pub workflow_id: String,
    /// Dispatch mode.
    pub mode: DispatchMode,
    /// Timeout in seconds (must be > 0).
    pub timeout_secs: u64,
    pub environment_profile: String,

    // -- Optional actor fields --
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,

    // -- Optional dispatch fields --
    /// Authentication mode — defaults to `ApiKey` if absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_mode: Option<AuthMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Non-secret environment variables for the dispatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_env: Option<HashMap<String, String>>,
    /// Secret names required at runtime (not values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_secrets: Option<Vec<String>>,
    /// Whether to preserve the environment on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_on_failure: Option<bool>,
}

/// Filter parameters for listing dispatches.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchListFilter {
    /// Filter by dispatch status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DispatchStatus>,
    /// Filter by concurrency lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<Lane>,
    /// Filter by project name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Maximum number of results to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    /// Number of results to skip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

/// Request to cancel a dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CancelDispatchRequest {
    /// UUID of the dispatch to cancel.
    pub dispatch_id: Uuid,
    /// Organization UUID (required for actor attribution).
    pub org_id: Uuid,
    /// User UUID (required for actor attribution).
    pub user_id: Uuid,
    /// Team UUID (optional actor attribution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<Uuid>,
    /// Reason for cancellation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
