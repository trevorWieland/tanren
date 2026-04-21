use std::fmt;

use serde::{Deserialize, Serialize};
use tanren_domain::{
    Cli, DispatchId, ExecuteResult, Finding, FiniteF64, NonEmptyString, Outcome, Phase, StepId,
    TimeoutSecs, TokenUsage,
};

use crate::capability::HarnessRequirements;

/// Normalized harness execution request.
///
/// Secret values are carried only for in-process redaction and are never
/// persisted as part of runtime/domain events.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessExecutionRequest {
    pub dispatch_id: DispatchId,
    pub step_id: StepId,
    pub cli: Cli,
    pub phase: Phase,
    pub timeout_secs: TimeoutSecs,
    pub working_directory: NonEmptyString,
    pub prompt: String,
    #[serde(default)]
    pub requirements: HarnessRequirements,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_secret_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secret_values_for_redaction: Vec<String>,
}

impl fmt::Debug for HarnessExecutionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HarnessExecutionRequest")
            .field("dispatch_id", &self.dispatch_id)
            .field("step_id", &self.step_id)
            .field("cli", &self.cli)
            .field("phase", &self.phase)
            .field("timeout_secs", &self.timeout_secs)
            .field("working_directory", &self.working_directory)
            .field("prompt_len", &self.prompt.len())
            .field("requirements", &self.requirements)
            .field("required_secret_names", &self.required_secret_names)
            .field(
                "secret_values_for_redaction_count",
                &self.secret_values_for_redaction.len(),
            )
            .finish()
    }
}

/// Raw output emitted by a harness adapter before redaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawExecutionOutput {
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub duration_secs: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
    #[serde(default)]
    pub pushed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_hash: Option<String>,
    #[serde(default)]
    pub unchecked_tasks: u32,
    #[serde(default)]
    pub spec_modified: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

/// Output that is safe to persist in domain events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistableOutput {
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub duration_secs: FiniteF64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
    #[serde(default)]
    pub pushed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_hash: Option<String>,
    #[serde(default)]
    pub unchecked_tasks: u32,
    #[serde(default)]
    pub spec_modified: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

impl PersistableOutput {
    #[must_use]
    pub fn into_execute_result(self) -> ExecuteResult {
        ExecuteResult {
            outcome: self.outcome,
            signal: self.signal,
            exit_code: self.exit_code,
            duration_secs: self.duration_secs,
            gate_output: self.gate_output,
            tail_output: self.tail_output,
            stderr_tail: self.stderr_tail,
            pushed: self.pushed,
            plan_hash: self.plan_hash,
            unchecked_tasks: self.unchecked_tasks,
            spec_modified: self.spec_modified,
            findings: self.findings,
            token_usage: self.token_usage,
        }
    }
}

/// Normalized terminal result returned by the harness contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessExecutionResult {
    pub output: PersistableOutput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_run_id: Option<String>,
    #[serde(default)]
    pub session_resumed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_debug_redacts_secret_values() {
        let request = HarnessExecutionRequest {
            dispatch_id: DispatchId::new(),
            step_id: StepId::new(),
            cli: Cli::Codex,
            phase: Phase::DoTask,
            timeout_secs: TimeoutSecs::try_new(60).expect("timeout"),
            working_directory: NonEmptyString::try_new("/tmp/work").expect("dir"),
            prompt: "ship it".into(),
            requirements: HarnessRequirements::default(),
            required_secret_names: vec!["API_TOKEN".into()],
            secret_values_for_redaction: vec!["sk-super-secret".into()],
        };
        let debug = format!("{request:?}");
        assert!(debug.contains("secret_values_for_redaction_count"));
        assert!(!debug.contains("sk-super-secret"));
    }

    #[test]
    fn persistable_output_converts_to_domain_execute_result() {
        let output = PersistableOutput {
            outcome: Outcome::Success,
            signal: None,
            exit_code: Some(0),
            duration_secs: FiniteF64::try_new(1.25).expect("finite"),
            gate_output: Some("ok".into()),
            tail_output: Some("done".into()),
            stderr_tail: None,
            pushed: true,
            plan_hash: Some("abc123".into()),
            unchecked_tasks: 0,
            spec_modified: true,
            findings: vec![],
            token_usage: Some(TokenUsage::default()),
        };
        let execute = output.into_execute_result();
        assert_eq!(execute.outcome, Outcome::Success);
        assert_eq!(execute.tail_output.as_deref(), Some("done"));
    }
}
