use crate::adapter::{
    HarnessAdapter, HarnessContractError, HarnessEventSource, HarnessExecutionEvent,
    HarnessExecutionEventKind, HarnessObserver, ProviderMetadataViolation, execute_with_contract,
};
use crate::execution::HarnessExecutionRequest;
use crate::failure::{
    HarnessFailureClass, ProviderFailureCode, ProviderFailureContext, classify_provider_failure,
};
use crate::redaction::{default_redaction_policy, scanner};

/// Minimal result wrapper for reusable conformance checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceResult {
    pub events: Vec<HarnessExecutionEvent>,
}

/// Additional conformance assertions for redaction outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RedactionConformanceExpectations {
    pub required_absent_fragments: Vec<String>,
    pub required_present_fragments: Vec<String>,
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
    adapter: &impl HarnessAdapter,
    request: &HarnessExecutionRequest,
) -> Result<ConformanceResult, String> {
    let mut recorder = ConformanceEventRecorder::default();
    let err = execute_with_contract(adapter, request, &mut recorder).await;
    let denial_kind = match err {
        Err(HarnessContractError::CompatibilityDenied(denial)) => denial.kind,
        Err(other) => return Err(format!("expected compatibility denial, got {other}")),
        Ok(_) => {
            return Err("expected compatibility denial but execution succeeded".to_string());
        }
    };

    let denied = recorder.events().iter().any(|event| {
        event.source == HarnessEventSource::Contract
            && matches!(
                event.kind,
                HarnessExecutionEventKind::PreflightDenied(kind) if kind == denial_kind
            )
    });
    if !denied {
        return Err(format!(
            "missing PreflightDenied({denial_kind:?}) contract event"
        ));
    }

    if recorder.events().iter().any(|event| {
        event.source == HarnessEventSource::Contract
            && matches!(event.kind, HarnessExecutionEventKind::PreflightAccepted)
    }) {
        return Err("preflight accepted event emitted for denied request".into());
    }

    if recorder.events().iter().any(|event| {
        event.source == HarnessEventSource::Contract
            && matches!(event.kind, HarnessExecutionEventKind::AdapterInvoked)
    }) {
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
    adapter: &impl HarnessAdapter,
    request: &HarnessExecutionRequest,
    expectations: &RedactionConformanceExpectations,
) -> Result<ConformanceResult, String> {
    let policy = default_redaction_policy();
    let mut recorder = ConformanceEventRecorder::default();
    let result = execute_with_contract(adapter, request, &mut recorder)
        .await
        .map_err(|err| err.to_string())?;

    assert_event_ordering(recorder.events())?;

    let hints = request.redaction_hints();
    let channels = [
        result.output.gate_output.as_deref(),
        result.output.tail_output.as_deref(),
        result.output.stderr_tail.as_deref(),
    ];

    for secret in &hints.secret_values {
        let secret = secret.expose();
        if secret.trim().is_empty() {
            continue;
        }
        if channels.iter().flatten().any(|text| text.contains(secret)) {
            return Err("persistable output leaked explicit secret value".into());
        }
    }

    for key in &hints.required_secret_names {
        if channels
            .iter()
            .flatten()
            .any(|text| scanner::contains_unredacted_assignment(text, key.as_str(), "[REDACTED]"))
        {
            return Err(format!(
                "persistable output leaked unredacted key assignment for `{}`",
                key.as_str()
            ));
        }
    }

    if channels.iter().flatten().any(|text| {
        scanner::contains_unredacted_bearer_token(text, policy.min_token_len, "[REDACTED]")
    }) {
        return Err("persistable output leaked bearer-style token".into());
    }

    for prefix in &policy.token_prefixes {
        if channels.iter().flatten().any(|text| {
            scanner::contains_unredacted_prefixed_token(
                text,
                prefix,
                policy.min_token_len,
                "[REDACTED]",
            )
        }) {
            return Err(format!(
                "persistable output leaked token with sensitive prefix `{prefix}`"
            ));
        }
    }
    for fragment in &expectations.required_absent_fragments {
        if channels
            .iter()
            .flatten()
            .any(|text| text.contains(fragment.as_str()))
        {
            return Err(format!(
                "persistable output retained forbidden fragment `{fragment}`"
            ));
        }
    }

    for fragment in &expectations.required_present_fragments {
        if !channels
            .iter()
            .flatten()
            .any(|text| text.contains(fragment.as_str()))
        {
            return Err(format!(
                "persistable output removed required benign fragment `{fragment}`"
            ));
        }
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

/// Assert that terminal typed-code semantics are deterministic and stable.
///
/// # Errors
/// Returns a message when the code-to-class mapping does not match.
pub fn assert_terminal_typed_code_mapping(
    typed_code: ProviderFailureCode,
    expected: HarnessFailureClass,
) -> Result<(), String> {
    let ctx = ProviderFailureContext::new(typed_code);
    assert_failure_classification(&ctx, expected)
}

/// Assert that unsafe provider metadata is rejected fail-closed.
///
/// # Errors
/// Returns a message describing the violated conformance rule.
pub async fn assert_provider_metadata_fail_closed(
    adapter: &impl HarnessAdapter,
    request: &HarnessExecutionRequest,
    field: &'static str,
) -> Result<ConformanceResult, String> {
    let mut recorder = ConformanceEventRecorder::default();
    let err = execute_with_contract(adapter, request, &mut recorder).await;
    let violation = match err {
        Err(HarnessContractError::UnsafeProviderMetadata {
            field: actual_field,
            violation,
        }) if actual_field == field => violation,
        Err(other) => {
            return Err(format!(
                "expected UnsafeProviderMetadata for `{field}`, got {other}"
            ));
        }
        Ok(_) => return Err("expected unsafe provider metadata failure".to_string()),
    };

    if !matches!(
        violation,
        ProviderMetadataViolation::RedactedOrMutated
            | ProviderMetadataViolation::InvalidFormat
            | ProviderMetadataViolation::LeakDetected
    ) {
        return Err(format!(
            "unexpected provider metadata violation for `{field}`: {violation:?}"
        ));
    }

    let accepted = event_index(recorder.events(), |event| {
        event.source == HarnessEventSource::Contract
            && matches!(event.kind, HarnessExecutionEventKind::PreflightAccepted)
    })
    .is_some();
    let invoked = event_index(recorder.events(), |event| {
        event.source == HarnessEventSource::Contract
            && matches!(event.kind, HarnessExecutionEventKind::AdapterInvoked)
    })
    .is_some();
    let persistable = event_index(recorder.events(), |event| {
        event.source == HarnessEventSource::Contract
            && matches!(
                event.kind,
                HarnessExecutionEventKind::PersistableOutputReady
            )
    })
    .is_some();

    if !(accepted && invoked && !persistable) {
        return Err("metadata fail-closed event ordering invariant violated".to_string());
    }

    Ok(ConformanceResult {
        events: recorder.events,
    })
}

fn assert_event_ordering(events: &[HarnessExecutionEvent]) -> Result<(), String> {
    let accepted = event_index(events, |event| {
        event.source == HarnessEventSource::Contract
            && matches!(event.kind, HarnessExecutionEventKind::PreflightAccepted)
    })
    .ok_or_else(|| "missing PreflightAccepted event".to_string())?;

    let invoked = event_index(events, |event| {
        event.source == HarnessEventSource::Contract
            && matches!(event.kind, HarnessExecutionEventKind::AdapterInvoked)
    })
    .ok_or_else(|| "missing AdapterInvoked event".to_string())?;

    let persistable = event_index(events, |event| {
        event.source == HarnessEventSource::Contract
            && matches!(
                event.kind,
                HarnessExecutionEventKind::PersistableOutputReady
            )
    })
    .ok_or_else(|| "missing PersistableOutputReady event".to_string())?;

    if !(accepted < invoked && invoked < persistable) {
        return Err("execution events are out of required order".into());
    }

    Ok(())
}

fn event_index(
    events: &[HarnessExecutionEvent],
    predicate: impl Fn(&HarnessExecutionEvent) -> bool,
) -> Option<usize> {
    events.iter().position(predicate)
}

#[cfg(test)]
mod tests;
