use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use tanren_domain::{
    Cli, DispatchId, FiniteF64, NonEmptyString, Outcome, Phase, StepId, TimeoutSecs,
};

use super::*;
use crate::adapter::{
    ExecutionSignal, HarnessContractError, HarnessExecutionEvent, HarnessExecutionEventKind,
};
use crate::capability::{
    ApprovalMode, HarnessCapabilities, HarnessRequirements, OutputStreaming, PatchApplyRequirement,
    PatchApplySupport, RequirementLevel, SandboxMode, SessionResumeRequirement,
    SessionResumeSupport,
};
use crate::execution::{RawExecutionOutput, RedactionSecret, SecretName};
use crate::failure::{
    ProviderFailure, ProviderFailureCode, ProviderFailureContext, ProviderIdentifier, ProviderRunId,
};

const FULL_CAPABILITIES: HarnessCapabilities = HarnessCapabilities {
    output_streaming: OutputStreaming::TextAndToolEvents,
    can_use_tools: true,
    patch_apply: PatchApplySupport::ApplyPatchAndUnifiedDiff,
    session_resume: SessionResumeSupport::CrossProcess,
    sandbox_mode: SandboxMode::WorkspaceWrite,
    approval_mode: ApprovalMode::OnDemand,
};

const TOOL_DENIED_CAPABILITIES: HarnessCapabilities = HarnessCapabilities {
    output_streaming: OutputStreaming::TextAndToolEvents,
    can_use_tools: false,
    patch_apply: PatchApplySupport::ApplyPatchAndUnifiedDiff,
    session_resume: SessionResumeSupport::CrossProcess,
    sandbox_mode: SandboxMode::WorkspaceWrite,
    approval_mode: ApprovalMode::OnDemand,
};

const LIMITED_LEVEL_CAPABILITIES: HarnessCapabilities = HarnessCapabilities {
    output_streaming: OutputStreaming::TextAndToolEvents,
    can_use_tools: true,
    patch_apply: PatchApplySupport::ApplyPatchOnly,
    session_resume: SessionResumeSupport::SameProcessOnly,
    sandbox_mode: SandboxMode::WorkspaceWrite,
    approval_mode: ApprovalMode::OnDemand,
};

#[derive(Debug, Clone)]
struct FullHarnessAdapter {
    raw_output: RawExecutionOutput,
    call_count: Arc<AtomicUsize>,
}

impl FullHarnessAdapter {
    fn new(raw_output: RawExecutionOutput) -> Self {
        Self {
            raw_output,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl HarnessAdapter for FullHarnessAdapter {
    fn adapter_name(&self) -> &'static str {
        "full"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        FULL_CAPABILITIES
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        _observer: &mut dyn HarnessObserver,
        _token: crate::adapter::ContractCallToken,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(ExecutionSignal {
            output: self.raw_output.clone(),
            provider_run_id: Some(ProviderRunId::try_new("mock-run").expect("run id")),
            session_resumed: false,
        })
    }
}

#[derive(Debug, Clone)]
struct PollutingHarnessAdapter {
    raw_output: RawExecutionOutput,
}

#[async_trait]
impl HarnessAdapter for PollutingHarnessAdapter {
    fn adapter_name(&self) -> &'static str {
        "polluting-mock"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        FULL_CAPABILITIES
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        observer: &mut dyn HarnessObserver,
        _token: crate::adapter::ContractCallToken,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        observer.on_event(HarnessExecutionEvent::contract(
            HarnessExecutionEventKind::PersistableOutputReady,
        ));
        Ok(ExecutionSignal {
            output: self.raw_output.clone(),
            provider_run_id: Some(ProviderRunId::try_new("polluting-run").expect("run id")),
            session_resumed: false,
        })
    }
}

#[derive(Debug, Clone)]
struct ToolDeniedHarnessAdapter {
    raw_output: RawExecutionOutput,
    call_count: Arc<AtomicUsize>,
}

impl ToolDeniedHarnessAdapter {
    fn new(raw_output: RawExecutionOutput) -> Self {
        Self {
            raw_output,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl HarnessAdapter for ToolDeniedHarnessAdapter {
    fn adapter_name(&self) -> &'static str {
        "tool-denied"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        TOOL_DENIED_CAPABILITIES
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        _observer: &mut dyn HarnessObserver,
        _token: crate::adapter::ContractCallToken,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(ExecutionSignal {
            output: self.raw_output.clone(),
            provider_run_id: Some(ProviderRunId::try_new("tool-denied-run").expect("run id")),
            session_resumed: false,
        })
    }
}

#[derive(Debug, Clone)]
struct LimitedLevelsHarnessAdapter {
    raw_output: RawExecutionOutput,
    call_count: Arc<AtomicUsize>,
}

impl LimitedLevelsHarnessAdapter {
    fn new(raw_output: RawExecutionOutput) -> Self {
        Self {
            raw_output,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl HarnessAdapter for LimitedLevelsHarnessAdapter {
    fn adapter_name(&self) -> &'static str {
        "levels-limited"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        LIMITED_LEVEL_CAPABILITIES
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        _observer: &mut dyn HarnessObserver,
        _token: crate::adapter::ContractCallToken,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(ExecutionSignal {
            output: self.raw_output.clone(),
            provider_run_id: Some(ProviderRunId::try_new("levels-limited-run").expect("run id")),
            session_resumed: false,
        })
    }
}

#[derive(Debug, Clone)]
struct UnsafeMetadataHarnessAdapter {
    raw_output: RawExecutionOutput,
}

#[async_trait]
impl HarnessAdapter for UnsafeMetadataHarnessAdapter {
    fn adapter_name(&self) -> &'static str {
        "unsafe-metadata"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        FULL_CAPABILITIES
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        _observer: &mut dyn HarnessObserver,
        _token: crate::adapter::ContractCallToken,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        Ok(ExecutionSignal {
            output: self.raw_output.clone(),
            provider_run_id: Some(ProviderRunId::try_new("run-sk-live-secret").expect("run id")),
            session_resumed: false,
        })
    }
}

#[derive(Debug, Clone)]
struct UnsafeFailureMetadataHarnessAdapter;

#[async_trait]
impl HarnessAdapter for UnsafeFailureMetadataHarnessAdapter {
    fn adapter_name(&self) -> &'static str {
        "unsafe-failure-metadata"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        FULL_CAPABILITIES
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        _observer: &mut dyn HarnessObserver,
        _token: crate::adapter::ContractCallToken,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        Err(
            ProviderFailure::new(ProviderFailureCode::Authentication, "auth").with_context(
                ProviderFailureContext {
                    typed_code: ProviderFailureCode::Authentication,
                    provider_code: Some(
                        ProviderIdentifier::try_new("sk-live-secret").expect("identifier"),
                    ),
                    provider_kind: None,
                    signal: None,
                    exit_code: None,
                    stdout_tail: None,
                    stderr_tail: None,
                },
            ),
        )
    }
}

fn request(requirements: HarnessRequirements) -> HarnessExecutionRequest {
    HarnessExecutionRequest {
        dispatch_id: DispatchId::new(),
        step_id: StepId::new(),
        cli: Cli::Codex,
        phase: Phase::DoTask,
        timeout_secs: TimeoutSecs::try_new(60).expect("timeout"),
        working_directory: NonEmptyString::try_new("/tmp/work").expect("dir"),
        prompt: "perform work".into(),
        requirements,
        required_secret_names: vec![SecretName::try_new("API_TOKEN").expect("secret key")],
        secret_values_for_redaction: vec![RedactionSecret::from("line1-secret\nline2-secret")],
    }
}

fn raw_output_with_secret() -> RawExecutionOutput {
    RawExecutionOutput {
        outcome: Outcome::Success,
        signal: None,
        exit_code: Some(0),
        duration_secs: FiniteF64::try_new(1.2).expect("finite"),
        gate_output: Some("API_TOKEN=abc API_TOKEN='def' SAFE_MARKER".into()),
        tail_output: Some("Bearer sk-live-secret line1-secret AKIA123456789012345".into()),
        stderr_tail: Some("aws_secret_access_key=ghi line2-secret".into()),
        pushed: false,
        plan_hash: None,
        unchecked_tasks: 0,
        spec_modified: false,
        findings: vec![],
        token_usage: None,
    }
}

#[tokio::test]
async fn conformance_event_order_ignores_adapter_event_pollution() {
    let adapter = PollutingHarnessAdapter {
        raw_output: raw_output_with_secret(),
    };
    let req = request(HarnessRequirements::default());
    let expectations = RedactionConformanceExpectations {
        required_absent_fragments: vec!["abc".into(), "def".into(), "ghi".into()],
        required_present_fragments: vec!["SAFE_MARKER".into()],
    };

    let result = assert_redaction_before_persistence(&adapter, &req, &expectations).await;
    assert!(
        result.is_ok(),
        "{}",
        result
            .err()
            .unwrap_or_else(|| "expected conformance success".to_string())
    );
}

#[tokio::test]
async fn conformance_denies_before_adapter_side_effects() {
    let adapter = ToolDeniedHarnessAdapter::new(raw_output_with_secret());
    let req = request(
        HarnessRequirements::builder()
            .tool_use(RequirementLevel::Required)
            .build(),
    );
    let result = assert_capability_denial_is_preflight(&adapter, &req).await;
    assert!(
        result.is_ok(),
        "{}",
        result
            .err()
            .unwrap_or_else(|| "expected conformance success".to_string())
    );
    assert_eq!(adapter.calls(), 0);
}

#[tokio::test]
async fn conformance_enforces_redaction_before_persistence() {
    let adapter = FullHarnessAdapter::new(raw_output_with_secret());
    let req = request(HarnessRequirements::default());
    let expectations = RedactionConformanceExpectations {
        required_absent_fragments: vec![
            "abc".into(),
            "def".into(),
            "ghi".into(),
            "line1-secret".into(),
            "line2-secret".into(),
        ],
        required_present_fragments: vec!["SAFE_MARKER".into()],
    };
    let result = assert_redaction_before_persistence(&adapter, &req, &expectations).await;
    assert!(
        result.is_ok(),
        "{}",
        result
            .err()
            .unwrap_or_else(|| "expected conformance success".to_string())
    );
    assert_eq!(adapter.calls(), 1);
}

#[tokio::test]
async fn conformance_enforces_capability_levels() {
    let adapter = LimitedLevelsHarnessAdapter::new(raw_output_with_secret());
    let requirements = HarnessRequirements::builder()
        .patch_apply(PatchApplyRequirement::ApplyPatchAndUnifiedDiff)
        .session_resume(SessionResumeRequirement::CrossProcess)
        .build();
    let req = request(requirements);

    let result = assert_capability_denial_is_preflight(&adapter, &req).await;
    assert!(result.is_ok(), "must deny insufficient capability levels");
    assert_eq!(adapter.calls(), 0);
}

#[tokio::test]
async fn conformance_enforces_metadata_fail_closed_for_run_id() {
    let adapter = UnsafeMetadataHarnessAdapter {
        raw_output: raw_output_with_secret(),
    };
    let mut req = request(HarnessRequirements::default());
    req.secret_values_for_redaction = vec![RedactionSecret::from("sk-live-secret")];
    let result = assert_provider_metadata_fail_closed(&adapter, &req, "provider_run_id").await;
    assert!(
        result.is_ok(),
        "must fail closed for unsafe provider metadata"
    );
}

#[tokio::test]
async fn conformance_enforces_metadata_fail_closed_for_provider_code() {
    let adapter = UnsafeFailureMetadataHarnessAdapter;
    let req = request(HarnessRequirements::default());
    let result = assert_provider_metadata_fail_closed(&adapter, &req, "provider_code").await;
    assert!(
        result.is_ok(),
        "must fail closed for unsafe provider metadata"
    );
}

#[test]
fn conformance_classifies_failures() {
    let ctx = ProviderFailureContext {
        typed_code: ProviderFailureCode::RateLimited,
        provider_code: Some(ProviderIdentifier::try_new("429").expect("provider code")),
        provider_kind: None,
        signal: None,
        exit_code: None,
        stdout_tail: None,
        stderr_tail: None,
    };
    assert!(assert_failure_classification(&ctx, HarnessFailureClass::RateLimited).is_ok());
}

#[test]
fn conformance_enforces_terminal_typed_code_mapping() {
    assert!(
        assert_terminal_typed_code_mapping(
            ProviderFailureCode::TransportUnavailable,
            HarnessFailureClass::TransportUnavailable,
        )
        .is_ok()
    );
}

#[test]
fn conformance_asserts_dedicated_failure_path_leak_error() {
    assert!(
        assert_failure_path_leak_detected(&HarnessContractError::FailurePathRedactionLeakDetected)
            .is_ok()
    );
}

#[tokio::test]
async fn conformance_helpers_accept_dyn_adapter_objects() {
    let adapter: Box<dyn HarnessAdapter> =
        Box::new(FullHarnessAdapter::new(raw_output_with_secret()));
    let req = request(HarnessRequirements::default());
    let expectations = RedactionConformanceExpectations {
        required_absent_fragments: vec!["abc".into(), "def".into(), "ghi".into()],
        required_present_fragments: vec!["SAFE_MARKER".into()],
    };
    let result = assert_redaction_before_persistence(adapter.as_ref(), &req, &expectations).await;
    assert!(
        result.is_ok(),
        "dyn adapter should satisfy conformance checks"
    );
}

#[test]
fn conformance_event_ordering_requires_exact_contract_event_cardinality() {
    let events = vec![
        HarnessExecutionEvent::contract(HarnessExecutionEventKind::PreflightAccepted),
        HarnessExecutionEvent::contract(HarnessExecutionEventKind::PreflightAccepted),
        HarnessExecutionEvent::contract(HarnessExecutionEventKind::AdapterInvoked),
        HarnessExecutionEvent::contract(HarnessExecutionEventKind::PersistableOutputReady),
    ];

    let err = assert_event_ordering(&events).expect_err("duplicate contract events must fail");
    assert!(
        err.contains("expected exactly one matching contract event"),
        "unexpected error: {err}"
    );
}
