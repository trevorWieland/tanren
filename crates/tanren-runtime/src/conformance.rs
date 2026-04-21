use crate::adapter::{
    HarnessAdapter, HarnessContractError, HarnessExecutionEvent, HarnessObserver,
    execute_with_contract,
};
use crate::execution::HarnessExecutionRequest;
use crate::failure::{HarnessFailureClass, ProviderFailureContext, classify_provider_failure};
use crate::redaction::{DefaultOutputRedactor, RedactionHints};

/// Minimal result wrapper for reusable conformance checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceResult {
    pub events: Vec<HarnessExecutionEvent>,
}

/// Simple observer used by reusable conformance assertions.
#[derive(Debug, Default, Clone)]
pub struct ConformanceEventRecorder {
    events: Vec<HarnessExecutionEvent>,
}

impl ConformanceEventRecorder {
    #[must_use]
    pub fn events(&self) -> &[HarnessExecutionEvent] {
        &self.events
    }
}

impl HarnessObserver for ConformanceEventRecorder {
    fn on_event(&mut self, event: HarnessExecutionEvent) {
        self.events.push(event);
    }
}

/// Assert that an incompatible request is denied before adapter side effects.
///
/// # Errors
/// Returns a message describing the violated conformance rule.
pub async fn assert_capability_denial_is_preflight(
    adapter: &dyn HarnessAdapter,
    request: &HarnessExecutionRequest,
) -> Result<ConformanceResult, String> {
    let mut recorder = ConformanceEventRecorder::default();
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints::default();
    let err = execute_with_contract(adapter, request, &redactor, &hints, &mut recorder)
        .await
        .expect_err("request should be denied");
    match err {
        HarnessContractError::CompatibilityDenied(_) => {}
        other => return Err(format!("expected compatibility denial, got {other}")),
    }
    if recorder
        .events()
        .iter()
        .any(|event| matches!(event, HarnessExecutionEvent::AdapterInvoked))
    {
        return Err("adapter was invoked despite capability denial".into());
    }
    Ok(ConformanceResult {
        events: recorder.events,
    })
}

/// Assert that redaction runs before persistence and removes known secrets.
///
/// # Errors
/// Returns a message describing the violated conformance rule.
pub async fn assert_redaction_before_persistence(
    adapter: &dyn HarnessAdapter,
    request: &HarnessExecutionRequest,
    hints: &RedactionHints,
) -> Result<ConformanceResult, String> {
    let redactor = DefaultOutputRedactor::default();
    let mut recorder = ConformanceEventRecorder::default();
    let result = execute_with_contract(adapter, request, &redactor, hints, &mut recorder)
        .await
        .map_err(|err| err.to_string())?;
    for secret in &hints.secret_values {
        if secret.trim().is_empty() {
            continue;
        }
        if result
            .output
            .gate_output
            .as_deref()
            .is_some_and(|value| value.contains(secret))
            || result
                .output
                .tail_output
                .as_deref()
                .is_some_and(|value| value.contains(secret))
            || result
                .output
                .stderr_tail
                .as_deref()
                .is_some_and(|value| value.contains(secret))
        {
            return Err("persistable output leaked secret value".into());
        }
    }
    if !recorder
        .events()
        .iter()
        .any(|event| matches!(event, HarnessExecutionEvent::PersistableOutputReady))
    {
        return Err("persistable output event missing".into());
    }
    Ok(ConformanceResult {
        events: recorder.events,
    })
}

/// Assert that provider-failure classification maps to a stable typed class.
///
/// # Errors
/// Returns a message when the classification does not match.
pub fn assert_failure_classification(
    ctx: &ProviderFailureContext,
    expected: HarnessFailureClass,
) -> Result<(), String> {
    let actual = classify_provider_failure(ctx);
    if actual == expected {
        Ok(())
    } else {
        Err(format!("expected {expected:?}, got {actual:?}"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use tanren_domain::{Cli, DispatchId, NonEmptyString, Outcome, Phase, StepId, TimeoutSecs};

    use super::*;
    use crate::adapter::ExecutionSignal;
    use crate::capability::{
        ApprovalMode, HarnessCapabilities, HarnessRequirements, OutputStreaming, PatchApplySupport,
        SandboxMode, SessionResumeSupport,
    };
    use crate::execution::RawExecutionOutput;

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
        ) -> Result<ExecutionSignal, crate::failure::HarnessFailure> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(ExecutionSignal {
                output: self.raw_output.clone(),
                provider_run_id: Some("mock-run".into()),
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
            required_secret_names: vec!["API_TOKEN".into()],
            secret_values_for_redaction: vec!["line1-secret\nline2-secret".into()],
        }
    }

    fn raw_output_with_secret() -> RawExecutionOutput {
        RawExecutionOutput {
            outcome: Outcome::Success,
            signal: None,
            exit_code: Some(0),
            duration_secs: 1.2,
            gate_output: Some("API_TOKEN=abc line1-secret".into()),
            tail_output: Some("Bearer sk-live-secret line2-secret".into()),
            stderr_tail: Some("aws_secret_access_key=def".into()),
            pushed: false,
            plan_hash: None,
            unchecked_tasks: 0,
            spec_modified: false,
            findings: vec![],
            token_usage: None,
        }
    }

    #[tokio::test]
    async fn conformance_denies_before_adapter_side_effects() {
        let capabilities = HarnessCapabilities {
            can_use_tools: false,
            ..default_capabilities()
        };
        let adapter = MockHarnessAdapter::new(capabilities, raw_output_with_secret());
        let req = request(HarnessRequirements {
            tool_use: crate::capability::RequirementLevel::Required,
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
        let hints = RedactionHints {
            required_secret_names: vec!["API_TOKEN".into()],
            secret_values: vec!["line1-secret\nline2-secret".into()],
        };
        let result = assert_redaction_before_persistence(&adapter, &req, &hints).await;
        assert!(
            result.is_ok(),
            "{}",
            result
                .err()
                .unwrap_or_else(|| "expected conformance success".to_string())
        );
        assert_eq!(adapter.calls(), 1);
    }

    #[test]
    fn conformance_classifies_failures() {
        let ctx = ProviderFailureContext {
            stderr_tail: Some("429 too many requests".into()),
            ..ProviderFailureContext::default()
        };
        assert!(assert_failure_classification(&ctx, HarnessFailureClass::RateLimited).is_ok());
    }
}
