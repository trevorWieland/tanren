use crate::adapter::{
    HarnessAdapter, HarnessContractError, HarnessExecutionEvent, HarnessObserver,
    execute_with_contract,
};
use crate::execution::HarnessExecutionRequest;
use crate::failure::{HarnessFailureClass, ProviderFailureContext, classify_provider_failure};
use crate::redaction::{DefaultOutputRedactor, default_redaction_policy};

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
    adapter: &dyn HarnessAdapter,
    request: &HarnessExecutionRequest,
) -> Result<ConformanceResult, String> {
    let mut recorder = ConformanceEventRecorder::default();
    let redactor = DefaultOutputRedactor::default();
    let err = execute_with_contract(adapter, request, &redactor, &mut recorder)
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
    expectations: &RedactionConformanceExpectations,
) -> Result<ConformanceResult, String> {
    let policy = default_redaction_policy();
    let redactor = DefaultOutputRedactor::new(policy.clone());
    let mut recorder = ConformanceEventRecorder::default();
    let result = execute_with_contract(adapter, request, &redactor, &mut recorder)
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
        if secret.trim().is_empty() {
            continue;
        }
        if channels
            .iter()
            .flatten()
            .any(|text| text.contains(secret.as_str()))
        {
            return Err("persistable output leaked explicit secret value".into());
        }
    }

    for key in &hints.required_secret_names {
        if channels
            .iter()
            .flatten()
            .any(|text| contains_unredacted_assignment(text, key))
        {
            return Err(format!(
                "persistable output leaked unredacted key assignment for `{key}`"
            ));
        }
    }

    if channels
        .iter()
        .flatten()
        .any(|text| contains_unredacted_bearer_token(text, policy.min_token_len))
    {
        return Err("persistable output leaked bearer-style token".into());
    }

    for prefix in &policy.token_prefixes {
        if channels
            .iter()
            .flatten()
            .any(|text| contains_unredacted_prefixed_token(text, prefix, policy.min_token_len))
        {
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

fn assert_event_ordering(events: &[HarnessExecutionEvent]) -> Result<(), String> {
    let accepted = event_index(events, |event| {
        matches!(event, HarnessExecutionEvent::PreflightAccepted)
    })
    .ok_or_else(|| "missing PreflightAccepted event".to_string())?;

    let invoked = event_index(events, |event| {
        matches!(event, HarnessExecutionEvent::AdapterInvoked)
    })
    .ok_or_else(|| "missing AdapterInvoked event".to_string())?;

    let persistable = event_index(events, |event| {
        matches!(event, HarnessExecutionEvent::PersistableOutputReady)
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

fn contains_unredacted_assignment(text: &str, key: &str) -> bool {
    let key_lower = key.to_ascii_lowercase();
    for line in text.lines() {
        let line_lower = line.to_ascii_lowercase();
        let mut search_from = 0;

        while let Some(offset) = line_lower[search_from..].find(&key_lower) {
            let key_start = search_from + offset;
            let key_end = key_start + key_lower.len();

            let mut cursor = key_end;
            while cursor < line.len() && line.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor >= line.len() || !matches!(line.as_bytes()[cursor], b'=' | b':') {
                search_from = key_end;
                continue;
            }
            cursor += 1;
            while cursor < line.len() && line.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor >= line.len() {
                return false;
            }

            let (value_start, value_end) = if matches!(line.as_bytes()[cursor], b'"' | b'\'') {
                let quoted_end = find_quoted_end(line, cursor).unwrap_or(line.len());
                (cursor.saturating_add(1), quoted_end)
            } else {
                (cursor, find_unquoted_end(line, cursor))
            };

            let value = &line[value_start..value_end];
            if !value.starts_with("[REDACTED]") {
                return true;
            }

            search_from = value_end.saturating_add(1);
        }
    }
    false
}

fn contains_unredacted_bearer_token(text: &str, min_token_len: usize) -> bool {
    let mut search_from = 0;
    while let Some(index) = find_ascii_case_insensitive(text, "bearer ", search_from) {
        let token_start = index + "bearer ".len();
        let token_end = find_unquoted_end(text, token_start);
        if token_end.saturating_sub(token_start) >= min_token_len {
            let token = &text[token_start..token_end];
            if token != "[REDACTED]" {
                return true;
            }
        }
        search_from = token_end.saturating_add(1);
    }
    false
}

fn contains_unredacted_prefixed_token(text: &str, prefix: &str, min_token_len: usize) -> bool {
    let mut search_from = 0;
    while let Some(offset) = text[search_from..].find(prefix) {
        let start = search_from + offset;
        let mut end = start + prefix.len();
        while end < text.len() {
            let ch = text.as_bytes()[end];
            if !(ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b'/' | b'+' | b'=')) {
                break;
            }
            end += 1;
        }

        if end.saturating_sub(start) >= min_token_len && &text[start..end] != "[REDACTED]" {
            return true;
        }

        search_from = end.saturating_add(1);
    }
    false
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str, start: usize) -> Option<usize> {
    if needle.is_empty() || start >= haystack.len() {
        return None;
    }

    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if haystack_bytes.len() < needle_bytes.len() {
        return None;
    }

    let mut idx = start;
    while idx + needle_bytes.len() <= haystack_bytes.len() {
        let mut match_all = true;
        for offset in 0..needle_bytes.len() {
            if !haystack_bytes[idx + offset].eq_ignore_ascii_case(&needle_bytes[offset]) {
                match_all = false;
                break;
            }
        }
        if match_all {
            return Some(idx);
        }
        idx += 1;
    }

    None
}

fn find_quoted_end(value: &str, start: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    if start >= bytes.len() {
        return None;
    }
    let quote = bytes[start];
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == quote && bytes[cursor.saturating_sub(1)] != b'\\' {
            return Some(cursor);
        }
        cursor += 1;
    }
    Some(value.len())
}

fn find_unquoted_end(value: &str, start: usize) -> usize {
    let bytes = value.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        let ch = bytes[cursor];
        if ch.is_ascii_whitespace() || ch == b',' || ch == b';' {
            break;
        }
        cursor += 1;
    }
    cursor
}

#[cfg(test)]
mod tests;
