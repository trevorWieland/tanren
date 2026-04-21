use async_trait::async_trait;
use std::sync::OnceLock;

use crate::capability::{CompatibilityDenial, CompatibilityDenialKind, HarnessCapabilities};
use crate::execution::{HarnessExecutionRequest, HarnessExecutionResult, RawExecutionOutput};
use crate::failure::{HarnessFailure, ProviderFailure};
use crate::redaction::{DefaultOutputRedactor, OutputRedactor, RedactionError};

/// Events emitted by the contract wrapper and adapters during execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessExecutionEventKind {
    PreflightAccepted,
    PreflightDenied(CompatibilityDenialKind),
    AdapterInvoked,
    PersistableOutputReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessEventSource {
    Contract,
    Adapter,
}

/// Events emitted by the contract wrapper and adapters during execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessExecutionEvent {
    pub source: HarnessEventSource,
    pub kind: HarnessExecutionEventKind,
}

impl HarnessExecutionEvent {
    #[must_use]
    pub const fn contract(kind: HarnessExecutionEventKind) -> Self {
        Self {
            source: HarnessEventSource::Contract,
            kind,
        }
    }

    #[must_use]
    pub const fn with_source(mut self, source: HarnessEventSource) -> Self {
        self.source = source;
        self
    }
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
    /// Returns [`ProviderFailure`] if the provider returns a terminal failure.
    async fn execute(
        &self,
        request: &HarnessExecutionRequest,
        observer: &mut dyn HarnessObserver,
    ) -> Result<ExecutionSignal, ProviderFailure>;
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
    static DEFAULT_OUTPUT_REDACTOR: OnceLock<DefaultOutputRedactor> = OnceLock::new();
    let redactor = DEFAULT_OUTPUT_REDACTOR.get_or_init(DefaultOutputRedactor::default);
    execute_with_contract_internal(adapter, request, redactor, observer).await
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
            emit_contract_event(
                observer,
                HarnessExecutionEventKind::PreflightDenied(err.kind),
            );
            HarnessContractError::CompatibilityDenied(err)
        })?;
    emit_contract_event(observer, HarnessExecutionEventKind::PreflightAccepted);
    emit_contract_event(observer, HarnessExecutionEventKind::AdapterInvoked);

    let mut adapter_observer = AdapterObserverProxy { inner: observer };
    let signal = adapter
        .execute(request, &mut adapter_observer)
        .await
        .map_err(HarnessFailure::from_provider_failure)
        .map_err(HarnessContractError::AdapterFailure)?;
    let hints = request.redaction_hints();
    let output = redactor.redact(signal.output, &hints)?;
    if redactor.has_known_secret_leak(&output, &hints)
        || redactor.has_policy_residual_leak(&output, &hints)
    {
        return Err(HarnessContractError::RedactionLeakDetected);
    }
    emit_contract_event(observer, HarnessExecutionEventKind::PersistableOutputReady);
    Ok(HarnessExecutionResult {
        output,
        provider_run_id: signal.provider_run_id,
        session_resumed: signal.session_resumed,
    })
}

fn emit_contract_event(observer: &mut dyn HarnessObserver, kind: HarnessExecutionEventKind) {
    observer.on_event(HarnessExecutionEvent::contract(kind));
}

struct AdapterObserverProxy<'a> {
    inner: &'a mut dyn HarnessObserver,
}

impl HarnessObserver for AdapterObserverProxy<'_> {
    fn on_event(&mut self, event: HarnessExecutionEvent) {
        self.inner
            .on_event(event.with_source(HarnessEventSource::Adapter));
    }
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
