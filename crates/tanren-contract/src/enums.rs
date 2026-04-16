//! Contract-owned enum types for interface schemas.
//!
//! These enums intentionally live in `tanren-contract` so transport
//! surfaces depend on contract types, not domain types.
//!
//! The contract crate is transport-neutral. Clap's `ValueEnum`
//! derives, typer-style CLI parsers, and any other transport-specific
//! helpers belong in the transport binaries — see
//! `bin/tanren-cli/src/commands/enums.rs` for the CLI wrappers.

use serde::{Deserialize, Serialize};

/// Dispatch mode for create requests and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchMode {
    Auto,
    Manual,
}

/// Dispatch lifecycle status for filters and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Concurrency lane for filters and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Impl,
    Audit,
    Gate,
}

/// Phase of work for create requests and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    DoTask,
    AuditTask,
    RunDemo,
    AuditSpec,
    Investigate,
    Gate,
    Setup,
    Cleanup,
}

/// CLI harness for create requests and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cli {
    Claude,
    Codex,
    #[serde(rename = "opencode")]
    OpenCode,
    Bash,
}

/// Authentication mode for create requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    #[default]
    ApiKey,
    #[serde(rename = "oauth")]
    OAuth,
    Subscription,
}

/// Dispatch outcome for responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Fail,
    Blocked,
    Error,
    Timeout,
}

/// Step kind for step responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Provision,
    Execute,
    Teardown,
    DryRun,
}

/// Step execution status for step responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Scheduler-ready state for step responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepReadyState {
    Blocked,
    Ready,
}
