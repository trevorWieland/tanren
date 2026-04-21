use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use crate::capability::{CompatibilityDenial, CompatibilityDenialKind, HarnessCapabilities};
use crate::execution::{HarnessExecutionRequest, HarnessExecutionResult, RawExecutionOutput};
use crate::failure::{HarnessFailure, ProviderFailure, ProviderIdentifier};
use crate::redaction::{DefaultOutputRedactor, OutputRedactor, RedactionError, RedactionHints};

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

mod call_token {
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct Seal;
}

/// Contract-owned proof token required to invoke adapter execution.
///
/// This type is intentionally unconstructable outside crate internals.
///
/// ```compile_fail
/// use tanren_runtime::ContractCallToken;
///
/// let _token = ContractCallToken { };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ContractCallToken {
    _seal: call_token::Seal,
}

impl ContractCallToken {
    pub(crate) const fn new() -> Self {
        Self {
            _seal: call_token::Seal,
        }
    }
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
        token: ContractCallToken,
    ) -> Result<ExecutionSignal, ProviderFailure>;
}

/// Runtime registry for trait-object harness adapter instances.
#[derive(Default, Clone)]
pub struct HarnessAdapterRegistry {
    adapters: BTreeMap<&'static str, Arc<dyn HarnessAdapter>>,
}

impl std::fmt::Debug for HarnessAdapterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HarnessAdapterRegistry")
            .field("adapters", &self.names())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HarnessAdapterRegistryError {
    #[error("adapter `{name}` is already registered")]
    DuplicateAdapterName { name: &'static str },
}

impl HarnessAdapterRegistry {
    pub fn register(
        &mut self,
        adapter: Arc<dyn HarnessAdapter>,
    ) -> Result<(), HarnessAdapterRegistryError> {
        let name = adapter.adapter_name();
        if self.adapters.contains_key(name) {
            return Err(HarnessAdapterRegistryError::DuplicateAdapterName { name });
        }
        self.adapters.insert(name, adapter);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn HarnessAdapter> {
        self.adapters.get(name).map(Arc::as_ref)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.adapters.keys().copied().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderMetadataViolation {
    #[error("metadata value was redacted or mutated during sanitization")]
    RedactedOrMutated,
    #[error("metadata value violates the allowed format")]
    InvalidFormat,
    #[error("metadata value triggered redaction leak detection")]
    LeakDetected,
}

/// Contract-level failures surfaced to orchestrator/worker layers.
#[derive(Debug, thiserror::Error)]
pub enum HarnessContractError {
    #[error(transparent)]
    CompatibilityDenied(#[from] CompatibilityDenial),
    #[error(transparent)]
    AdapterFailure(#[from] HarnessFailure),
    #[error("unsafe provider metadata in `{field}`: {violation}")]
    UnsafeProviderMetadata {
        field: &'static str,
        violation: ProviderMetadataViolation,
    },
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
    let hints = request.redaction_hints();
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
    let signal = match adapter
        .execute(request, &mut adapter_observer, ContractCallToken::new())
        .await
    {
        Ok(signal) => signal,
        Err(provider_failure) => {
            let sanitized = sanitize_provider_failure(provider_failure, redactor, &hints)?;
            return Err(HarnessContractError::AdapterFailure(
                HarnessFailure::from_provider_failure(sanitized),
            ));
        }
    };
    let redaction = redactor.redact_with_audit(signal.output, &hints)?;
    if redaction.audit.has_any_leak() {
        return Err(HarnessContractError::RedactionLeakDetected);
    }
    let provider_run_id = sanitize_provider_run_id(signal.provider_run_id, redactor, &hints)?;
    emit_contract_event(observer, HarnessExecutionEventKind::PersistableOutputReady);
    Ok(HarnessExecutionResult {
        output: redaction.output,
        provider_run_id,
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

fn sanitize_provider_failure(
    mut failure: ProviderFailure,
    redactor: &dyn OutputRedactor,
    hints: &RedactionHints,
) -> Result<ProviderFailure, HarnessContractError> {
    let context = failure.context.clone();
    let sanitized = sanitize_failure_payload(
        redactor,
        hints,
        &failure.message,
        context.exit_code,
        context.stdout_tail.as_deref(),
        context.stderr_tail.as_deref(),
    )
    .map_err(HarnessContractError::Redaction)?;
    if sanitized.audit.has_any_leak() {
        return Err(HarnessContractError::Redaction(
            RedactionError::PolicyViolation,
        ));
    }
    failure.message = sanitized
        .output
        .gate_output
        .unwrap_or_else(|| "[REDACTED]".to_owned());
    failure.context.stdout_tail = sanitized.output.tail_output;
    failure.context.stderr_tail = sanitized.output.stderr_tail;
    failure.context.provider_code = sanitize_provider_identifier(
        "provider_code",
        failure.context.provider_code,
        redactor,
        hints,
    )?;
    failure.context.provider_kind = sanitize_provider_identifier(
        "provider_kind",
        failure.context.provider_kind,
        redactor,
        hints,
    )?;
    Ok(failure)
}

fn sanitize_provider_identifier(
    field: &'static str,
    provider_identifier: Option<ProviderIdentifier>,
    redactor: &dyn OutputRedactor,
    hints: &RedactionHints,
) -> Result<Option<ProviderIdentifier>, HarnessContractError> {
    let Some(raw_identifier) = provider_identifier else {
        return Ok(None);
    };

    let candidate = sanitize_metadata_value(
        field,
        raw_identifier.as_str(),
        ProviderIdentifier::MAX_LEN,
        redactor,
        hints,
    )?;
    let parsed = ProviderIdentifier::try_new(candidate).map_err(|_| {
        HarnessContractError::UnsafeProviderMetadata {
            field,
            violation: ProviderMetadataViolation::InvalidFormat,
        }
    })?;
    Ok(Some(parsed))
}

fn sanitize_provider_run_id(
    provider_run_id: Option<String>,
    redactor: &dyn OutputRedactor,
    hints: &RedactionHints,
) -> Result<Option<String>, HarnessContractError> {
    const MAX_PROVIDER_RUN_ID_LEN: usize = 128;

    let Some(raw_value) = provider_run_id else {
        return Ok(None);
    };

    let candidate = sanitize_metadata_value(
        "provider_run_id",
        &raw_value,
        MAX_PROVIDER_RUN_ID_LEN,
        redactor,
        hints,
    )?;
    Ok(Some(candidate))
}

fn sanitize_metadata_value(
    field: &'static str,
    raw_value: &str,
    max_len: usize,
    redactor: &dyn OutputRedactor,
    hints: &RedactionHints,
) -> Result<String, HarnessContractError> {
    let sanitized = sanitize_failure_payload(redactor, hints, raw_value, None, None, None)
        .map_err(HarnessContractError::Redaction)?;

    if sanitized.audit.has_any_leak() {
        return Err(HarnessContractError::UnsafeProviderMetadata {
            field,
            violation: ProviderMetadataViolation::LeakDetected,
        });
    }

    let Some(candidate) = sanitized.output.gate_output else {
        return Err(HarnessContractError::UnsafeProviderMetadata {
            field,
            violation: ProviderMetadataViolation::RedactedOrMutated,
        });
    };

    if candidate != raw_value {
        return Err(HarnessContractError::UnsafeProviderMetadata {
            field,
            violation: ProviderMetadataViolation::RedactedOrMutated,
        });
    }

    if !is_valid_provider_metadata_value(&candidate, max_len) {
        return Err(HarnessContractError::UnsafeProviderMetadata {
            field,
            violation: ProviderMetadataViolation::InvalidFormat,
        });
    }

    Ok(candidate)
}

fn is_valid_provider_metadata_value(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

fn sanitize_failure_payload(
    redactor: &dyn OutputRedactor,
    hints: &RedactionHints,
    gate_output: &str,
    exit_code: Option<i32>,
    tail_output: Option<&str>,
    stderr_tail: Option<&str>,
) -> Result<crate::redaction::RedactionOutcome, RedactionError> {
    let zero_duration =
        tanren_domain::FiniteF64::try_new(0.0).map_err(|_| RedactionError::PolicyViolation)?;
    let output = RawExecutionOutput {
        outcome: tanren_domain::Outcome::Error,
        signal: None,
        exit_code,
        duration_secs: zero_duration,
        gate_output: Some(gate_output.to_owned()),
        tail_output: tail_output.map(str::to_owned),
        stderr_tail: stderr_tail.map(str::to_owned),
        pushed: false,
        plan_hash: None,
        unchecked_tasks: 0,
        spec_modified: false,
        findings: Vec::new(),
        token_usage: None,
    };
    redactor.redact_with_audit(output, hints)
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
