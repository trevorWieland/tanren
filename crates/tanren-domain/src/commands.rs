//! Write-side command structs — inputs to the orchestrator.
//!
//! Every command carries explicit actor attribution ([`ActorContext`])
//! so policy and audit events can be attributed to the originating
//! org / user / team / API key without threading caller metadata through
//! transport layers separately.

use serde::{Deserialize, Serialize};

use crate::actor::ActorContext;
use crate::ids::{DispatchId, LeaseId, StepId};
use crate::payloads::{ConfigEnv, StepPayload};
use crate::policy::PolicyScope;
use crate::status::{AuthMode, Cli, DispatchMode, Lane, Phase, StepType};
use crate::validated::{NonEmptyString, TimeoutSecs};

/// Command to create a new dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDispatch {
    /// Actor / tenant attribution for this dispatch.
    pub actor: ActorContext,
    pub project: NonEmptyString,
    pub phase: Phase,
    pub cli: Cli,
    pub auth_mode: AuthMode,
    pub branch: NonEmptyString,
    pub spec_folder: NonEmptyString,
    pub workflow_id: NonEmptyString,
    pub mode: DispatchMode,
    pub timeout: TimeoutSecs,
    pub environment_profile: NonEmptyString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Non-secret project environment. Secrets must use `required_secrets`.
    #[serde(default)]
    pub project_env: ConfigEnv,
    /// References to secrets (by name) that must be injected at runtime.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_secrets: Vec<String>,
    #[serde(default)]
    pub preserve_on_failure: bool,
}

/// Command to enqueue a step within a dispatch.
///
/// Graph-native: callers supply the step's node identity, its graph
/// revision, and its dependency edges. Schedulers consult `depends_on`
/// to decide when the step becomes ready to dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnqueueStep {
    pub dispatch_id: DispatchId,
    /// Caller-provided node identity — enables deterministic replay and
    /// allows the planner to reference steps before they exist.
    pub step_id: StepId,
    pub step_type: StepType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<Lane>,
    /// Ordered list of step IDs this step depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<StepId>,
    /// Revision of the dispatch graph this step was enqueued under.
    /// Incremented by replans so stale enqueues can be detected.
    pub graph_revision: u32,
    pub payload: StepPayload,
}

/// Command to cancel a running or pending dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelDispatch {
    /// Actor initiating the cancellation — required for audit.
    pub actor: ActorContext,
    pub dispatch_id: DispatchId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Command to request an execution lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestLease {
    pub dispatch_id: DispatchId,
    pub step_id: StepId,
    pub capabilities: LeaseCapabilities,
    /// Typed policy scope for the lease request.
    pub policy_scope: PolicyScope,
}

/// Command to release an execution lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseLease {
    /// Actor initiating the release — required for audit.
    pub actor: ActorContext,
    pub lease_id: LeaseId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Capabilities requested for a lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseCapabilities {
    pub runtime_type: NonEmptyString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<ResourceLimits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mount_requirements: Vec<String>,
}

/// Resource limits for a lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory_mb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cpu_millicores: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_disk_mb: Option<u64>,
}
