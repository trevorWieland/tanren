//! Contract-owned enum types for interface schemas.
//!
//! These enums intentionally live in `tanren-contract` so transport
//! surfaces depend on contract types, not domain types.

use serde::{Deserialize, Serialize};

/// Dispatch mode for create requests and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum DispatchMode {
    Auto,
    Manual,
}

/// Dispatch lifecycle status for filters and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum DispatchStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Concurrency lane for filters and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum Lane {
    Impl,
    Audit,
    Gate,
}

/// Phase of work for create requests and responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum Cli {
    Claude,
    Codex,
    #[serde(rename = "opencode")]
    #[value(name = "opencode")]
    OpenCode,
    Bash,
}

/// Authentication mode for create requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum AuthMode {
    ApiKey,
    #[serde(rename = "oauth")]
    #[value(name = "oauth")]
    OAuth,
    Subscription,
}

/// Dispatch outcome for responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Fail,
    Blocked,
    Error,
    Timeout,
}
