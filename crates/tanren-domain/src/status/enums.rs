//! Value enums for dispatch configuration and classification.

use serde::{Deserialize, Serialize};

/// Whether the dispatcher auto-chains steps or the caller drives each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchMode {
    Auto,
    Manual,
}

impl std::fmt::Display for DispatchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::Manual => f.write_str("manual"),
        }
    }
}

/// The kind of work a step performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Provision,
    Execute,
    Teardown,
    DryRun,
}

impl std::fmt::Display for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provision => f.write_str("provision"),
            Self::Execute => f.write_str("execute"),
            Self::Teardown => f.write_str("teardown"),
            Self::DryRun => f.write_str("dry_run"),
        }
    }
}

/// Concurrency lane — steps in different lanes may execute in parallel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Impl,
    Audit,
    Gate,
}

impl std::fmt::Display for Lane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Impl => f.write_str("impl"),
            Self::Audit => f.write_str("audit"),
            Self::Gate => f.write_str("gate"),
        }
    }
}

/// The phase of work being performed within a dispatch.
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

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DoTask => f.write_str("do_task"),
            Self::AuditTask => f.write_str("audit_task"),
            Self::RunDemo => f.write_str("run_demo"),
            Self::AuditSpec => f.write_str("audit_spec"),
            Self::Investigate => f.write_str("investigate"),
            Self::Gate => f.write_str("gate"),
            Self::Setup => f.write_str("setup"),
            Self::Cleanup => f.write_str("cleanup"),
        }
    }
}

/// The CLI harness used to execute a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cli {
    Claude,
    Codex,
    /// Rendered as `"opencode"` (not `"open_code"`) to match the legacy
    /// Python wire format and the project's canonical spelling.
    #[serde(rename = "opencode")]
    OpenCode,
    Bash,
}

impl std::fmt::Display for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Claude => f.write_str("claude"),
            Self::Codex => f.write_str("codex"),
            Self::OpenCode => f.write_str("opencode"),
            Self::Bash => f.write_str("bash"),
        }
    }
}

/// Authentication mode for a dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    ApiKey,
    /// Rendered as `"oauth"` (not `"o_auth"`) to match the canonical tag.
    #[serde(rename = "oauth")]
    OAuth,
    Subscription,
}

impl std::fmt::Display for AuthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiKey => f.write_str("api_key"),
            Self::OAuth => f.write_str("oauth"),
            Self::Subscription => f.write_str("subscription"),
        }
    }
}

/// The outcome of an executed step or dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Fail,
    Blocked,
    Error,
    Timeout,
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => f.write_str("success"),
            Self::Fail => f.write_str("fail"),
            Self::Blocked => f.write_str("blocked"),
            Self::Error => f.write_str("error"),
            Self::Timeout => f.write_str("timeout"),
        }
    }
}

// EntityType was removed in favor of the typed `EntityRef` / `EntityKind`
// pair in `crate::entity` — the string-tag version didn't cover steps,
// leases, orgs, or projects, and kept envelope routing stringly-typed.

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert that a value's `Display` output round-trips through
    /// serde as a `snake_case` string tag. This catches drift between
    /// `Display::fmt` branches and `#[serde(rename_all = "snake_case")]`.
    fn assert_display_matches_serde<T>(value: T, expected: &str)
    where
        T: std::fmt::Display + Serialize,
    {
        assert_eq!(value.to_string(), expected);
        let json = serde_json::to_string(&value).expect("serialize");
        assert_eq!(json, format!("\"{expected}\""));
    }

    #[test]
    fn dispatch_mode_display() {
        assert_display_matches_serde(DispatchMode::Auto, "auto");
        assert_display_matches_serde(DispatchMode::Manual, "manual");
    }

    #[test]
    fn step_type_display() {
        assert_display_matches_serde(StepType::Provision, "provision");
        assert_display_matches_serde(StepType::Execute, "execute");
        assert_display_matches_serde(StepType::Teardown, "teardown");
        assert_display_matches_serde(StepType::DryRun, "dry_run");
    }

    #[test]
    fn lane_display() {
        assert_display_matches_serde(Lane::Impl, "impl");
        assert_display_matches_serde(Lane::Audit, "audit");
        assert_display_matches_serde(Lane::Gate, "gate");
    }

    #[test]
    fn phase_display() {
        assert_display_matches_serde(Phase::DoTask, "do_task");
        assert_display_matches_serde(Phase::AuditTask, "audit_task");
        assert_display_matches_serde(Phase::RunDemo, "run_demo");
        assert_display_matches_serde(Phase::AuditSpec, "audit_spec");
        assert_display_matches_serde(Phase::Investigate, "investigate");
        assert_display_matches_serde(Phase::Gate, "gate");
        assert_display_matches_serde(Phase::Setup, "setup");
        assert_display_matches_serde(Phase::Cleanup, "cleanup");
    }

    #[test]
    fn cli_display() {
        assert_display_matches_serde(Cli::Claude, "claude");
        assert_display_matches_serde(Cli::Codex, "codex");
        // Must match the canonical tag, not serde's auto-snake_case.
        assert_display_matches_serde(Cli::OpenCode, "opencode");
        assert_display_matches_serde(Cli::Bash, "bash");
    }

    #[test]
    fn auth_mode_display() {
        assert_display_matches_serde(AuthMode::ApiKey, "api_key");
        // Must match the canonical tag, not serde's auto-snake_case.
        assert_display_matches_serde(AuthMode::OAuth, "oauth");
        assert_display_matches_serde(AuthMode::Subscription, "subscription");
    }

    #[test]
    fn outcome_display() {
        assert_display_matches_serde(Outcome::Success, "success");
        assert_display_matches_serde(Outcome::Fail, "fail");
        assert_display_matches_serde(Outcome::Blocked, "blocked");
        assert_display_matches_serde(Outcome::Error, "error");
        assert_display_matches_serde(Outcome::Timeout, "timeout");
    }
}
