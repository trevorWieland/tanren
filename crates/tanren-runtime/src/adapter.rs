use async_trait::async_trait;

use crate::capability::{CompatibilityDenial, CompatibilityDenialKind, HarnessCapabilities};
use crate::execution::{HarnessExecutionRequest, HarnessExecutionResult, RawExecutionOutput};
use crate::failure::HarnessFailure;
use crate::redaction::{DefaultOutputRedactor, OutputRedactor, RedactionError};

/// Events emitted by the contract wrapper and adapters during execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessExecutionEvent {
    PreflightAccepted,
    PreflightDenied(CompatibilityDenialKind),
    AdapterInvoked,
    PersistableOutputReady,
}

/// Observer for execution events.
pub trait HarnessObserver: Send {
    fn on_event(&mut self, event: HarnessExecutionEvent);
}

/// Result returned by concrete adapter implementations before redaction.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionSignal {
    pub output: RawExecutionOutput,
    pub provider_run_id: Option<String>,
    pub session_resumed: bool,
}

/// Adapter trait for provider-specific harness integrations.
#[async_trait]
pub trait HarnessAdapter: Send + Sync {
    fn adapter_name(&self) -> &'static str;

    fn capabilities(&self) -> HarnessCapabilities;

    /// Execute one request and return raw output prior to redaction.
    ///
    /// # Errors
    /// Returns [`HarnessFailure`] if the provider returns a terminal failure.
    async fn execute(
        &self,
        request: &HarnessExecutionRequest,
        observer: &mut dyn HarnessObserver,
    ) -> Result<ExecutionSignal, HarnessFailure>;
}

/// Contract-level failures surfaced to orchestrator/worker layers.
#[derive(Debug, thiserror::Error)]
pub enum HarnessContractError {
    #[error(transparent)]
    CompatibilityDenied(#[from] CompatibilityDenial),
    #[error(transparent)]
    AdapterFailure(#[from] HarnessFailure),
    #[error(transparent)]
    Redaction(#[from] RedactionError),
    #[error("redaction leak detected in persistable output")]
    RedactionLeakDetected,
}

/// Execute through the contract wrapper:
/// 1. capability preflight
/// 2. adapter execution
/// 3. redaction before persistence
/// 4. leak-check on known secret material
///
/// # Errors
/// Returns [`HarnessContractError`] for any failed stage.
pub async fn execute_with_contract(
    adapter: &dyn HarnessAdapter,
    request: &HarnessExecutionRequest,
    observer: &mut dyn HarnessObserver,
) -> Result<HarnessExecutionResult, HarnessContractError> {
    let redactor = DefaultOutputRedactor::default();
    execute_with_contract_internal(adapter, request, &redactor, observer).await
}

async fn execute_with_contract_internal(
    adapter: &dyn HarnessAdapter,
    request: &HarnessExecutionRequest,
    redactor: &dyn OutputRedactor,
    observer: &mut dyn HarnessObserver,
) -> Result<HarnessExecutionResult, HarnessContractError> {
    adapter
        .capabilities()
        .ensure_admissible(&request.requirements)
        .map_err(|err| {
            observer.on_event(HarnessExecutionEvent::PreflightDenied(err.kind));
            HarnessContractError::CompatibilityDenied(err)
        })?;
    observer.on_event(HarnessExecutionEvent::PreflightAccepted);
    observer.on_event(HarnessExecutionEvent::AdapterInvoked);

    let signal = adapter.execute(request, observer).await?;
    let hints = request.redaction_hints();
    let output = redactor.redact(signal.output, &hints)?;
    if redactor.has_known_secret_leak(&output, &hints) {
        return Err(HarnessContractError::RedactionLeakDetected);
    }
    observer.on_event(HarnessExecutionEvent::PersistableOutputReady);
    Ok(HarnessExecutionResult {
        output,
        provider_run_id: signal.provider_run_id,
        session_resumed: signal.session_resumed,
    })
}

#[cfg(test)]
pub(crate) async fn execute_with_contract_for_tests(
    adapter: &dyn HarnessAdapter,
    request: &HarnessExecutionRequest,
    redactor: &dyn OutputRedactor,
    observer: &mut dyn HarnessObserver,
) -> Result<HarnessExecutionResult, HarnessContractError> {
    execute_with_contract_internal(adapter, request, redactor, observer).await
}

#[cfg(test)]
mod tests;
