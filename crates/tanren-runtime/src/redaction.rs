use serde::{Deserialize, Serialize};
use tanren_domain::FiniteF64;

use crate::execution::{PersistableOutput, RawExecutionOutput};

const REDACTION_TOKEN: &str = "[REDACTED]";

/// Inputs used by redaction policy at capture-time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionHints {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_secret_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secret_values: Vec<String>,
}

/// Deterministic redaction policy used by all adapters unless overridden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionPolicy {
    pub min_token_len: usize,
    pub min_secret_fragment_len: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensitive_key_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_prefixes: Vec<String>,
}

/// Build the default redaction policy for Phase 1 harness adapters.
#[must_use]
pub fn default_redaction_policy() -> RedactionPolicy {
    RedactionPolicy {
        min_token_len: 10,
        min_secret_fragment_len: 4,
        sensitive_key_names: vec![
            "api_key".into(),
            "api-token".into(),
            "api_token".into(),
            "auth_token".into(),
            "access_token".into(),
            "refresh_token".into(),
            "session_token".into(),
            "authorization".into(),
            "bearer".into(),
            "cookie".into(),
            "set-cookie".into(),
            "password".into(),
            "secret".into(),
            "private_key".into(),
            "aws_access_key_id".into(),
            "aws_secret_access_key".into(),
            "x-api-key".into(),
        ],
        token_prefixes: vec![
            "sk-".into(),
            "ghp_".into(),
            "gho_".into(),
            "xoxb-".into(),
            "xoxp-".into(),
            "AKIA".into(),
        ],
    }
}

/// Redaction failure classes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RedactionError {
    #[error("execution duration is non-finite")]
    InvalidDuration,
}

/// Redacts raw harness output into persistable output.
pub trait OutputRedactor: Send + Sync {
    /// Apply redaction and normalize the output for durable persistence.
    ///
    /// # Errors
    /// Returns [`RedactionError`] if output cannot be normalized safely.
    fn redact(
        &self,
        output: RawExecutionOutput,
        hints: &RedactionHints,
    ) -> Result<PersistableOutput, RedactionError>;

    /// Detect whether the normalized output still leaks known secret values.
    #[must_use]
    fn has_known_secret_leak(&self, output: &PersistableOutput, hints: &RedactionHints) -> bool;
}

/// Default policy-driven output redactor.
#[derive(Debug, Clone)]
pub struct DefaultOutputRedactor {
    policy: RedactionPolicy,
}

impl Default for DefaultOutputRedactor {
    fn default() -> Self {
        Self {
            policy: default_redaction_policy(),
        }
    }
}

impl DefaultOutputRedactor {
    #[must_use]
    pub fn new(policy: RedactionPolicy) -> Self {
        Self { policy }
    }

    fn redact_text(&self, input: Option<String>, hints: &RedactionHints) -> Option<String> {
        input.map(|value| {
            let mut out = value;
            out = self.redact_structured_key_values(out, hints);
            out = self.redact_bearer_tokens(out);
            out = self.redact_prefixed_tokens(out);
            out = self.redact_explicit_secret_values(out, hints);
            out
        })
    }

    fn redact_structured_key_values(&self, input: String, hints: &RedactionHints) -> String {
        let mut keys = self.policy.sensitive_key_names.clone();
        for value in &hints.required_secret_names {
            keys.push(value.to_ascii_lowercase());
        }
        let mut out = String::with_capacity(input.len());
        for line in input.split_inclusive('\n') {
            let mut redacted_line = line.to_string();
            for key in &keys {
                redacted_line = redact_assignment_for_key(&redacted_line, key);
            }
            out.push_str(&redacted_line);
        }
        if out.is_empty() { input } else { out }
    }

    fn redact_bearer_tokens(&self, input: String) -> String {
        redact_keyword_token(input, "bearer ", self.policy.min_token_len)
    }

    fn redact_prefixed_tokens(&self, input: String) -> String {
        let mut out = input;
        for prefix in &self.policy.token_prefixes {
            out = redact_prefixed_token(out, prefix, self.policy.min_token_len);
        }
        out
    }

    fn redact_explicit_secret_values(&self, input: String, hints: &RedactionHints) -> String {
        let mut out = input;
        for value in &hints.secret_values {
            if value.trim().is_empty() {
                continue;
            }
            out = out.replace(value, REDACTION_TOKEN);
            if value.contains('\n') {
                for fragment in value.lines() {
                    if fragment.trim().len() >= self.policy.min_secret_fragment_len {
                        out = out.replace(fragment.trim(), REDACTION_TOKEN);
                    }
                }
            }
        }
        out
    }

    fn any_field_contains_secret(output: &PersistableOutput, secret: &str) -> bool {
        output
            .gate_output
            .as_deref()
            .is_some_and(|v| v.contains(secret))
            || output
                .tail_output
                .as_deref()
                .is_some_and(|v| v.contains(secret))
            || output
                .stderr_tail
                .as_deref()
                .is_some_and(|v| v.contains(secret))
    }
}

impl OutputRedactor for DefaultOutputRedactor {
    fn redact(
        &self,
        output: RawExecutionOutput,
        hints: &RedactionHints,
    ) -> Result<PersistableOutput, RedactionError> {
        let duration_secs = FiniteF64::try_new(output.duration_secs)
            .map_err(|_| RedactionError::InvalidDuration)?;
        Ok(PersistableOutput {
            outcome: output.outcome,
            signal: output.signal,
            exit_code: output.exit_code,
            duration_secs,
            gate_output: self.redact_text(output.gate_output, hints),
            tail_output: self.redact_text(output.tail_output, hints),
            stderr_tail: self.redact_text(output.stderr_tail, hints),
            pushed: output.pushed,
            plan_hash: output.plan_hash,
            unchecked_tasks: output.unchecked_tasks,
            spec_modified: output.spec_modified,
            findings: output.findings,
            token_usage: output.token_usage,
        })
    }

    fn has_known_secret_leak(&self, output: &PersistableOutput, hints: &RedactionHints) -> bool {
        for secret in &hints.secret_values {
            if secret.trim().is_empty() {
                continue;
            }
            if Self::any_field_contains_secret(output, secret) {
                return true;
            }
            if secret.contains('\n') {
                for fragment in secret.lines() {
                    if fragment.trim().len() >= self.policy.min_secret_fragment_len
                        && Self::any_field_contains_secret(output, fragment.trim())
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}

fn redact_assignment_for_key(line: &str, key: &str) -> String {
    let mut out = line.to_string();
    let key_lower = key.to_ascii_lowercase();
    let lower = out.to_ascii_lowercase();
    let Some(key_index) = lower.find(&key_lower) else {
        return out;
    };
    let mut cursor = key_index + key_lower.len();
    let bytes = out.as_bytes();
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    if cursor >= bytes.len() || (bytes[cursor] != b'=' && bytes[cursor] != b':') {
        return out;
    }
    cursor += 1;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    if cursor >= bytes.len() {
        return out;
    }
    let end = if bytes[cursor] == b'"' || bytes[cursor] == b'\'' {
        find_quoted_value_end(&out, cursor).unwrap_or(out.len())
    } else {
        find_unquoted_value_end(&out, cursor)
    };
    out.replace_range(cursor..end, REDACTION_TOKEN);
    out
}

fn find_quoted_value_end(value: &str, start: usize) -> Option<usize> {
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

fn find_unquoted_value_end(value: &str, start: usize) -> usize {
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

fn redact_keyword_token(mut value: String, keyword: &str, min_token_len: usize) -> String {
    let mut search_from = 0;
    loop {
        let lower = value.to_ascii_lowercase();
        let Some(offset) = lower[search_from..].find(keyword) else {
            return value;
        };
        let index = search_from + offset;
        let token_start = index + keyword.len();
        let token_end = find_unquoted_value_end(&value, token_start);
        if token_end.saturating_sub(token_start) < min_token_len {
            search_from = token_end.saturating_add(1);
            continue;
        }
        if &value[token_start..token_end] != REDACTION_TOKEN {
            value.replace_range(token_start..token_end, REDACTION_TOKEN);
        }
        search_from = token_start.saturating_add(REDACTION_TOKEN.len());
    }
}

fn redact_prefixed_token(mut value: String, prefix: &str, min_token_len: usize) -> String {
    let mut search_from = 0;
    loop {
        let Some(offset) = value[search_from..].find(prefix) else {
            return value;
        };
        let start = search_from + offset;
        let mut end = start + prefix.len();
        let bytes = value.as_bytes();
        while end < bytes.len() {
            let ch = bytes[end];
            if !(ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b'/' | b'+' | b'=')) {
                break;
            }
            end += 1;
        }
        if end.saturating_sub(start) < min_token_len {
            search_from = end.saturating_add(1);
            continue;
        }
        if &value[start..end] != REDACTION_TOKEN {
            value.replace_range(start..end, REDACTION_TOKEN);
        }
        search_from = start.saturating_add(REDACTION_TOKEN.len());
    }
}

#[cfg(test)]
mod tests {
    use tanren_domain::Outcome;

    use super::*;

    fn raw_output(text: &str) -> RawExecutionOutput {
        RawExecutionOutput {
            outcome: Outcome::Success,
            signal: None,
            exit_code: Some(0),
            duration_secs: 1.0,
            gate_output: Some(text.into()),
            tail_output: Some(text.into()),
            stderr_tail: Some(text.into()),
            pushed: false,
            plan_hash: None,
            unchecked_tasks: 0,
            spec_modified: false,
            findings: vec![],
            token_usage: None,
        }
    }

    #[test]
    fn redacts_bearer_and_prefixed_tokens() {
        let redactor = DefaultOutputRedactor::default();
        let hints = RedactionHints::default();
        let out = redactor
            .redact(
                raw_output("Authorization: Bearer sk-super-long-secret-123"),
                &hints,
            )
            .expect("redact");
        let value = out.gate_output.expect("output");
        assert!(!value.contains("sk-super-long-secret-123"));
        assert!(value.contains(REDACTION_TOKEN));
    }

    #[test]
    fn redacts_explicit_secret_values_and_multiline_fragments() {
        let redactor = DefaultOutputRedactor::default();
        let hints = RedactionHints {
            required_secret_names: vec!["MY_SECRET".into()],
            secret_values: vec!["line1-secret\nline2-secret".into()],
        };
        let out = redactor
            .redact(
                raw_output("line1-secret / line2-secret / MY_SECRET=abc"),
                &hints,
            )
            .expect("redact");
        let gate = out.gate_output.expect("gate");
        assert!(!gate.contains("line1-secret"));
        assert!(!gate.contains("line2-secret"));
        assert!(!gate.contains("abc"));
    }

    #[test]
    fn leak_detection_flags_remaining_secret() {
        let redactor = DefaultOutputRedactor::default();
        let hints = RedactionHints {
            required_secret_names: vec![],
            secret_values: vec!["secret-value".into()],
        };
        let output = PersistableOutput {
            outcome: Outcome::Success,
            signal: None,
            exit_code: None,
            duration_secs: FiniteF64::try_new(1.0).expect("finite"),
            gate_output: Some("still has secret-value".into()),
            tail_output: None,
            stderr_tail: None,
            pushed: false,
            plan_hash: None,
            unchecked_tasks: 0,
            spec_modified: false,
            findings: vec![],
            token_usage: None,
        };
        assert!(redactor.has_known_secret_leak(&output, &hints));
    }

    #[test]
    fn rejects_non_finite_duration() {
        let redactor = DefaultOutputRedactor::default();
        let err = redactor
            .redact(
                RawExecutionOutput {
                    duration_secs: f64::NAN,
                    ..raw_output("ok")
                },
                &RedactionHints::default(),
            )
            .expect_err("must fail");
        assert_eq!(err, RedactionError::InvalidDuration);
    }
}
