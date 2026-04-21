use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use tanren_domain::{
    Cli, DispatchId, FiniteF64, NonEmptyString, Outcome, Phase, StepId, TimeoutSecs,
};

use super::*;
use crate::adapter::{ExecutionSignal, HarnessExecutionEvent, HarnessExecutionEventKind};
use crate::capability::{
    ApprovalMode, HarnessCapabilities, HarnessRequirements, OutputStreaming, PatchApplyRequirement,
    PatchApplySupport, RequirementLevel, SandboxMode, SessionResumeRequirement,
    SessionResumeSupport,
};
use crate::execution::{RawExecutionOutput, RedactionSecret, SecretName};
use crate::failure::{ProviderFailure, ProviderIdentifier};

#[derive(Debug, Clone)]
struct MockHarnessAdapter {
    capabilities: HarnessCapabilities,
    raw_output: RawExecutionOutput,
    call_count: Arc<AtomicUsize>,
}

impl MockHarnessAdapter {
    fn new(capabilities: HarnessCapabilities, raw_output: RawExecutionOutput) -> Self {
        Self {
            capabilities,
            raw_output,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl HarnessAdapter for MockHarnessAdapter {
    fn adapter_name(&self) -> &'static str {
        "mock"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        self.capabilities.clone()
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        _observer: &mut dyn HarnessObserver,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(ExecutionSignal {
            output: self.raw_output.clone(),
            provider_run_id: Some("mock-run".into()),
            session_resumed: false,
        })
    }
}

#[derive(Debug, Clone)]
struct PollutingHarnessAdapter {
    capabilities: HarnessCapabilities,
    raw_output: RawExecutionOutput,
}

#[async_trait]
impl HarnessAdapter for PollutingHarnessAdapter {
    fn adapter_name(&self) -> &'static str {
        "polluting-mock"
    }

    fn capabilities(&self) -> HarnessCapabilities {
        self.capabilities.clone()
    }

    async fn execute(
        &self,
        _request: &HarnessExecutionRequest,
        observer: &mut dyn HarnessObserver,
    ) -> Result<ExecutionSignal, ProviderFailure> {
        observer.on_event(HarnessExecutionEvent::contract(
            HarnessExecutionEventKind::PersistableOutputReady,
        ));
        Ok(ExecutionSignal {
            output: self.raw_output.clone(),
            provider_run_id: Some("polluting-run".into()),
            session_resumed: false,
        })
    }
}

fn default_capabilities() -> HarnessCapabilities {
    HarnessCapabilities {
        output_streaming: OutputStreaming::TextAndToolEvents,
        can_use_tools: true,
        patch_apply: PatchApplySupport::ApplyPatchAndUnifiedDiff,
        session_resume: SessionResumeSupport::CrossProcess,
        sandbox_mode: SandboxMode::WorkspaceWrite,
        approval_mode: ApprovalMode::OnDemand,
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
        capabilities: default_capabilities(),
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
    let capabilities = HarnessCapabilities {
        can_use_tools: false,
        ..default_capabilities()
    };
    let adapter = MockHarnessAdapter::new(capabilities, raw_output_with_secret());
    let req = request(HarnessRequirements {
        tool_use: RequirementLevel::Required,
        ..HarnessRequirements::default()
    });
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
    let adapter = MockHarnessAdapter::new(default_capabilities(), raw_output_with_secret());
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
    let capabilities = HarnessCapabilities {
        patch_apply: PatchApplySupport::ApplyPatchOnly,
        session_resume: SessionResumeSupport::SameProcessOnly,
        ..default_capabilities()
    };
    let adapter = MockHarnessAdapter::new(capabilities, raw_output_with_secret());
    let req = request(HarnessRequirements {
        patch_apply: PatchApplyRequirement::ApplyPatchAndUnifiedDiff,
        session_resume: SessionResumeRequirement::CrossProcess,
        ..HarnessRequirements::default()
    });

    let result = assert_capability_denial_is_preflight(&adapter, &req).await;
    assert!(result.is_ok(), "must deny insufficient capability levels");
    assert_eq!(adapter.calls(), 0);
}

#[test]
fn conformance_classifies_failures() {
    let ctx = ProviderFailureContext {
        provider_code: Some(ProviderIdentifier::try_new("429").expect("provider code")),
        ..ProviderFailureContext::default()
    };
    assert!(assert_failure_classification(&ctx, HarnessFailureClass::RateLimited).is_ok());
}
