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
use crate::failure::{ProviderFailure, ProviderFailureCode, ProviderFailureContext};
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
    provider_failure: Option<ProviderFailure>,
}

#[async_trait]
impl HarnessAdapter for MockAdapter {
    const CAPABILITIES: HarnessCapabilities = HarnessCapabilities {
        output_streaming: OutputStreaming::TextAndToolEvents,
        can_use_tools: true,
        patch_apply: PatchApplySupport::ApplyPatchAndUnifiedDiff,
        session_resume: SessionResumeSupport::CrossProcess,
        sandbox_mode: SandboxMode::WorkspaceWrite,
        approval_mode: ApprovalMode::OnDemand,
    };

    fn adapter_name(&self) -> &'static str {
        "mock"
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        _observer: &mut dyn HarnessObserver,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        if let Some(failure) = &self.provider_failure {
            return Err(failure.clone());
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
            duration_secs: output.duration_secs,
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

    fn has_policy_residual_leak(
        &self,
        _output: &PersistableOutput,
        _hints: &RedactionHints,
    ) -> bool {
        false
    }
}

struct ErrorRedactor;

impl OutputRedactor for ErrorRedactor {
    fn redact(
        &self,
        _output: RawExecutionOutput,
        _hints: &RedactionHints,
    ) -> Result<PersistableOutput, RedactionError> {
        Err(RedactionError::PolicyViolation)
    }

    fn has_known_secret_leak(&self, _output: &PersistableOutput, _hints: &RedactionHints) -> bool {
        false
    }

    fn has_policy_residual_leak(
        &self,
        _output: &PersistableOutput,
        _hints: &RedactionHints,
    ) -> bool {
        false
    }
}

struct PolicyLeakRedactor;

impl OutputRedactor for PolicyLeakRedactor {
    fn redact(
        &self,
        output: RawExecutionOutput,
        _hints: &RedactionHints,
    ) -> Result<PersistableOutput, RedactionError> {
        Ok(PersistableOutput {
            outcome: output.outcome,
            signal: output.signal,
            exit_code: output.exit_code,
            duration_secs: output.duration_secs,
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
        false
    }

    fn has_policy_residual_leak(
        &self,
        _output: &PersistableOutput,
        _hints: &RedactionHints,
    ) -> bool {
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
        duration_secs: FiniteF64::try_new(1.0).expect("finite"),
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

#[test]
fn capabilities_are_immutable_type_metadata() {
    let capabilities = MockAdapter::CAPABILITIES;
    assert!(capabilities.can_use_tools);
    assert_eq!(capabilities.sandbox_mode, SandboxMode::WorkspaceWrite);
}

#[tokio::test]
async fn emits_expected_event_sequence_for_adapter_failure() {
    let adapter = MockAdapter {
        output: raw_output(),
        provider_failure: Some(ProviderFailure::new("adapter failed")),
    };
    let mut recorder = Recorder::default();
    let err = execute_with_contract(&adapter, &request(), &mut recorder)
        .await
        .expect_err("must fail");
    assert!(matches!(err, HarnessContractError::AdapterFailure(_)));
    assert_eq!(
        recorder.events,
        vec![
            HarnessExecutionEvent::contract(HarnessExecutionEventKind::PreflightAccepted),
            HarnessExecutionEvent::contract(HarnessExecutionEventKind::AdapterInvoked)
        ]
    );
}

#[tokio::test]
async fn normalizes_adapter_provider_failure_in_contract_wrapper() {
    let adapter = MockAdapter {
        output: raw_output(),
        provider_failure: Some(ProviderFailure::new("provider timeout").with_context(
            ProviderFailureContext {
                typed_code: Some(ProviderFailureCode::Timeout),
                stderr_tail: Some("401 invalid api key".into()),
                ..ProviderFailureContext::default()
            },
        )),
    };
    let mut recorder = Recorder::default();
    let err = execute_with_contract(&adapter, &request(), &mut recorder)
        .await
        .expect_err("must fail");
    assert!(matches!(err, HarnessContractError::AdapterFailure(_)));
    let HarnessContractError::AdapterFailure(failure) = err else {
        unreachable!("checked by matches assertion");
    };
    assert_eq!(
        failure.class(),
        crate::failure::HarnessFailureClass::Timeout
    );
    assert_eq!(failure.typed_code(), Some(ProviderFailureCode::Timeout));
}

#[tokio::test]
async fn sanitizes_failure_message_and_context_before_surface() {
    let adapter = MockAdapter {
        output: raw_output(),
        provider_failure: Some(
            ProviderFailure::new("request failed with API_TOKEN=super-secret and sk-live-secret")
                .with_context(ProviderFailureContext {
                    typed_code: Some(ProviderFailureCode::Authentication),
                    stdout_tail: Some("stdout secret sk-live-secret".into()),
                    stderr_tail: Some("stderr secret API_TOKEN=super-secret".into()),
                    ..ProviderFailureContext::default()
                }),
        ),
    };
    let mut req = request();
    req.secret_values_for_redaction = vec![RedactionSecret::from("super-secret")];
    req.required_secret_names = vec![SecretName::try_new("API_TOKEN").expect("secret key")];
    let mut recorder = Recorder::default();
    let err = execute_with_contract(&adapter, &req, &mut recorder)
        .await
        .expect_err("must fail");
    let HarnessContractError::AdapterFailure(failure) = err else {
        unreachable!("checked by expect_err");
    };
    assert!(!failure.message().contains("super-secret"));
    assert!(!failure.message().contains("sk-live-secret"));
    assert!(failure.message().contains("[REDACTED]"));
    assert_eq!(
        failure.class(),
        crate::failure::HarnessFailureClass::Authentication
    );
}

#[tokio::test]
async fn emits_expected_event_sequence_for_redaction_error() {
    let adapter = MockAdapter {
        output: raw_output(),
        provider_failure: None,
    };
    let mut recorder = Recorder::default();
    let err = execute_with_contract_for_tests(&adapter, &request(), &ErrorRedactor, &mut recorder)
        .await
        .expect_err("must fail");
    assert!(matches!(err, HarnessContractError::Redaction(_)));
    assert_eq!(
        recorder.events,
        vec![
            HarnessExecutionEvent::contract(HarnessExecutionEventKind::PreflightAccepted),
            HarnessExecutionEvent::contract(HarnessExecutionEventKind::AdapterInvoked)
        ]
    );
}

#[tokio::test]
async fn emits_expected_event_sequence_for_leak_detection() {
    let adapter = MockAdapter {
        output: raw_output(),
        provider_failure: None,
    };
    let mut recorder = Recorder::default();
    let err = execute_with_contract_for_tests(&adapter, &request(), &LeakRedactor, &mut recorder)
        .await
        .expect_err("must fail");
    assert!(matches!(err, HarnessContractError::RedactionLeakDetected));
    assert_eq!(
        recorder.events,
        vec![
            HarnessExecutionEvent::contract(HarnessExecutionEventKind::PreflightAccepted),
            HarnessExecutionEvent::contract(HarnessExecutionEventKind::AdapterInvoked)
        ]
    );
}

#[tokio::test]
async fn emits_expected_event_sequence_for_policy_residual_leak_detection() {
    let adapter = MockAdapter {
        output: raw_output(),
        provider_failure: None,
    };
    let mut recorder = Recorder::default();
    let err =
        execute_with_contract_for_tests(&adapter, &request(), &PolicyLeakRedactor, &mut recorder)
            .await
            .expect_err("must fail");
    assert!(matches!(err, HarnessContractError::RedactionLeakDetected));
    assert_eq!(
        recorder.events,
        vec![
            HarnessExecutionEvent::contract(HarnessExecutionEventKind::PreflightAccepted),
            HarnessExecutionEvent::contract(HarnessExecutionEventKind::AdapterInvoked)
        ]
    );
}
