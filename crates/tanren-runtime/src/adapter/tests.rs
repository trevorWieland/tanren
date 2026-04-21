use async_trait::async_trait;
use tanren_domain::{
    Cli, DispatchId, FiniteF64, NonEmptyString, Outcome, Phase, StepId, TimeoutSecs,
};

use super::*;
use crate::capability::{
    ApprovalMode, HarnessCapabilities, HarnessRequirements, OutputStreaming, PatchApplySupport,
    SandboxMode, SessionResumeSupport,
};
use crate::execution::{PersistableOutput, RawExecutionOutput, RedactionSecret, SecretName};
use crate::failure::{HarnessFailure, HarnessFailureClass};
use crate::redaction::{OutputRedactor, RedactionError, RedactionHints};

#[derive(Default)]
struct Recorder {
    events: Vec<HarnessExecutionEvent>,
}

impl HarnessObserver for Recorder {
    fn on_event(&mut self, event: HarnessExecutionEvent) {
        self.events.push(event);
    }
}

#[derive(Clone)]
struct MockAdapter {
    output: RawExecutionOutput,
    should_fail: bool,
}

#[async_trait]
impl HarnessAdapter for MockAdapter {
    fn adapter_name(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        HarnessCapabilities {
            output_streaming: OutputStreaming::TextAndToolEvents,
            can_use_tools: true,
            patch_apply: PatchApplySupport::ApplyPatchAndUnifiedDiff,
            session_resume: SessionResumeSupport::CrossProcess,
            sandbox_mode: SandboxMode::WorkspaceWrite,
            approval_mode: ApprovalMode::OnDemand,
        }
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        _observer: &mut dyn HarnessObserver,
    ) -> Result<ExecutionSignal, HarnessFailure> {
        if self.should_fail {
            return Err(HarnessFailure {
                class: HarnessFailureClass::Fatal,
                message: "adapter failed".into(),
                provider_code: None,
                provider_kind: None,
                typed_code: None,
            });
        }
        Ok(ExecutionSignal {
            output: self.output.clone(),
            provider_run_id: None,
            session_resumed: false,
        })
    }
}

struct LeakRedactor;

impl OutputRedactor for LeakRedactor {
    fn redact(
        &self,
        output: RawExecutionOutput,
        _hints: &RedactionHints,
    ) -> Result<PersistableOutput, RedactionError> {
        Ok(PersistableOutput {
            outcome: output.outcome,
            signal: output.signal,
            exit_code: output.exit_code,
            duration_secs: FiniteF64::try_new(output.duration_secs)
                .map_err(|_| RedactionError::InvalidDuration)?,
            gate_output: output.gate_output,
            tail_output: output.tail_output,
            stderr_tail: output.stderr_tail,
            pushed: output.pushed,
            plan_hash: output.plan_hash,
            unchecked_tasks: output.unchecked_tasks,
            spec_modified: output.spec_modified,
            findings: output.findings,
            token_usage: output.token_usage,
        })
    }

    fn has_known_secret_leak(&self, _output: &PersistableOutput, _hints: &RedactionHints) -> bool {
        true
    }
}

fn request() -> HarnessExecutionRequest {
    HarnessExecutionRequest {
        dispatch_id: DispatchId::new(),
        step_id: StepId::new(),
        cli: Cli::Codex,
        phase: Phase::DoTask,
        timeout_secs: TimeoutSecs::try_new(60).expect("timeout"),
        working_directory: NonEmptyString::try_new("/tmp/work").expect("dir"),
        prompt: "perform work".into(),
        requirements: HarnessRequirements::default(),
        required_secret_names: vec![SecretName::try_new("API_TOKEN").expect("secret key")],
        secret_values_for_redaction: vec![RedactionSecret::from("sk-live-secret")],
    }
}

fn raw_output() -> RawExecutionOutput {
    RawExecutionOutput {
        outcome: Outcome::Success,
        signal: None,
        exit_code: Some(0),
        duration_secs: 1.0,
        gate_output: Some("safe".into()),
        tail_output: Some("safe".into()),
        stderr_tail: Some("safe".into()),
        pushed: false,
        plan_hash: None,
        unchecked_tasks: 0,
        spec_modified: false,
        findings: vec![],
        token_usage: None,
    }
}

#[tokio::test]
async fn emits_expected_event_sequence_for_adapter_failure() {
    let adapter = MockAdapter {
        output: raw_output(),
        should_fail: true,
    };
    let mut recorder = Recorder::default();
    let err = execute_with_contract(&adapter, &request(), &mut recorder)
        .await
        .expect_err("must fail");
    assert!(matches!(err, HarnessContractError::AdapterFailure(_)));
    assert_eq!(
        recorder.events,
        vec![
            HarnessExecutionEvent::PreflightAccepted,
            HarnessExecutionEvent::AdapterInvoked
        ]
    );
}

#[tokio::test]
async fn emits_expected_event_sequence_for_redaction_error() {
    let adapter = MockAdapter {
        output: RawExecutionOutput {
            duration_secs: f64::NAN,
            ..raw_output()
        },
        should_fail: false,
    };
    let mut recorder = Recorder::default();
    let err = execute_with_contract(&adapter, &request(), &mut recorder)
        .await
        .expect_err("must fail");
    assert!(matches!(err, HarnessContractError::Redaction(_)));
    assert_eq!(
        recorder.events,
        vec![
            HarnessExecutionEvent::PreflightAccepted,
            HarnessExecutionEvent::AdapterInvoked
        ]
    );
}

#[tokio::test]
async fn emits_expected_event_sequence_for_leak_detection() {
    let adapter = MockAdapter {
        output: raw_output(),
        should_fail: false,
    };
    let mut recorder = Recorder::default();
    let err = execute_with_contract_for_tests(&adapter, &request(), &LeakRedactor, &mut recorder)
        .await
        .expect_err("must fail");
    assert!(matches!(err, HarnessContractError::RedactionLeakDetected));
    assert_eq!(
        recorder.events,
        vec![
            HarnessExecutionEvent::PreflightAccepted,
            HarnessExecutionEvent::AdapterInvoked
        ]
    );
}
