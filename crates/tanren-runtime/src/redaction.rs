use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use tanren_domain::FiniteF64;

use crate::execution::{PersistableOutput, RawExecutionOutput, RedactionSecret, SecretName};

pub(crate) mod scanner;

const REDACTION_TOKEN: &str = "[REDACTED]";
const TRUNCATION_MARKER: &str = "[TRUNCATED_FOR_PERSISTENCE]";

/// Inputs used by redaction policy at capture-time.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct RedactionHints {
    pub required_secret_names: Vec<SecretName>,
    pub secret_values: Vec<RedactionSecret>,
}

impl fmt::Debug for RedactionHints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedactionHints")
            .field("required_secret_names", &self.required_secret_names)
            .field("secret_value_count", &self.secret_values.len())
            .finish()
    }
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
    pub max_persistable_channel_bytes: usize,
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
        max_persistable_channel_bytes: 512 * 1024,
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
    normalized_sensitive_key_names: HashSet<String>,
}

impl Default for DefaultOutputRedactor {
    fn default() -> Self {
        Self::new(default_redaction_policy())
    }
}

impl DefaultOutputRedactor {
    #[must_use]
    pub fn new(policy: RedactionPolicy) -> Self {
        let normalized_sensitive_key_names = policy
            .sensitive_key_names
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect();
        Self {
            policy,
            normalized_sensitive_key_names,
        }
    }

    fn redact_text(&self, input: Option<String>, hints: &RedactionHints) -> Option<String> {
        input.map(|value| {
            let mut out =
                truncate_for_persistence(value, self.policy.max_persistable_channel_bytes);
            out = self.redact_structured_key_values(out, hints);
            out = self.redact_bearer_tokens(out);
            out = self.redact_prefixed_tokens(out);
            out = self.redact_explicit_secret_values(out, hints);
            out
        })
    }

    fn redact_structured_key_values(&self, input: String, hints: &RedactionHints) -> String {
        let hint_key_names = hints
            .required_secret_names
            .iter()
            .map(SecretName::as_str)
            .map(str::to_owned)
            .collect::<HashSet<_>>();

        let mut out = String::with_capacity(input.len());
        for line in input.split_inclusive('\n') {
            let redacted_line = redact_assignments_for_keys(
                line,
                &self.normalized_sensitive_key_names,
                &hint_key_names,
            );
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
        for secret in &hints.secret_values {
            let value = secret.expose();
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
            let secret = secret.expose();
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

fn redact_assignments_for_keys(
    line: &str,
    policy_keys: &HashSet<String>,
    hint_keys: &HashSet<String>,
) -> String {
    let bytes = line.as_bytes();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        if !is_key_start(bytes[cursor]) {
            cursor += 1;
            continue;
        }

        let key_start = cursor;
        cursor += 1;
        while cursor < bytes.len() && is_key_char(bytes[cursor]) {
            cursor += 1;
        }
        let key_end = cursor;

        let mut value_cursor = cursor;
        while value_cursor < bytes.len() && bytes[value_cursor].is_ascii_whitespace() {
            value_cursor += 1;
        }
        if value_cursor >= bytes.len() || !matches!(bytes[value_cursor], b'=' | b':') {
            continue;
        }

        let key = line[key_start..key_end].to_ascii_lowercase();
        let is_sensitive = policy_keys.contains(&key) || hint_keys.contains(&key);

        value_cursor += 1;
        while value_cursor < bytes.len() && bytes[value_cursor].is_ascii_whitespace() {
            value_cursor += 1;
        }
        if value_cursor >= bytes.len() {
            break;
        }

        let (value_start, value_end) = if matches!(bytes[value_cursor], b'"' | b'\'') {
            let quoted_end =
                scanner::find_quoted_value_end(line, value_cursor).unwrap_or(line.len());
            (value_cursor.saturating_add(1), quoted_end)
        } else {
            (
                value_cursor,
                scanner::find_unquoted_value_end(line, value_cursor),
            )
        };

        if is_sensitive
            && value_start < value_end
            && &line[value_start..value_end] != REDACTION_TOKEN
        {
            ranges.push((value_start, value_end));
        }

        cursor = value_end.saturating_add(1);
    }

    if ranges.is_empty() {
        return line.to_owned();
    }

    let mut out = line.to_owned();
    for (start, end) in ranges.into_iter().rev() {
        out.replace_range(start..end, REDACTION_TOKEN);
    }
    out
}

const fn is_key_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_key_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn redact_keyword_token(mut value: String, keyword: &str, min_token_len: usize) -> String {
    let mut search_from = 0;
    loop {
        let Some(index) = scanner::find_ascii_case_insensitive(&value, keyword, search_from) else {
            return value;
        };

        let token_start = index + keyword.len();
        let token_end = scanner::find_unquoted_value_end(&value, token_start);
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
        let Some(start) = scanner::find_ascii_case_insensitive(&value, prefix, search_from) else {
            return value;
        };
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

fn truncate_for_persistence(mut input: String, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input;
    }

    if max_bytes == 0 {
        return TRUNCATION_MARKER.to_owned();
    }

    let cutoff = nearest_char_boundary(&input, max_bytes);
    let boundary = nearest_delimiter_before(&input, cutoff);
    input.truncate(boundary);
    if !input.ends_with('\n') {
        input.push('\n');
    }
    input.push_str(TRUNCATION_MARKER);
    input
}

fn nearest_char_boundary(value: &str, preferred: usize) -> usize {
    if preferred >= value.len() {
        return value.len();
    }
    let mut idx = preferred;
    while idx > 0 && !value.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn nearest_delimiter_before(value: &str, cutoff: usize) -> usize {
    let floor = cutoff.saturating_sub(256);
    let mut idx = cutoff;
    while idx > floor {
        let Some(ch) = value[..idx].chars().next_back() else {
            break;
        };
        if ch.is_ascii_whitespace() || matches!(ch, ',' | ';' | '|' | '&') {
            return idx;
        }
        idx = idx.saturating_sub(ch.len_utf8());
    }
    cutoff
}

#[cfg(test)]
mod tests;
