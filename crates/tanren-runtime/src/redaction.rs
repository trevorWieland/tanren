use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::execution::{PersistableOutput, RawExecutionOutput, RedactionSecret, SecretName};

pub(crate) mod policy_dataset;
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
pub const fn default_redaction_policy_dataset_version() -> &'static str {
    policy_dataset::DEFAULT_REDACTION_POLICY_DATASET_VERSION
}

/// Build the default redaction policy for Phase 1 harness adapters.
#[must_use]
pub fn default_redaction_policy() -> RedactionPolicy {
    let dataset = policy_dataset::default_policy_dataset_v1();
    RedactionPolicy {
        min_token_len: dataset.min_token_len,
        min_secret_fragment_len: dataset.min_secret_fragment_len,
        sensitive_key_names: dataset
            .sensitive_key_names
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        token_prefixes: dataset
            .token_prefixes
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        max_persistable_channel_bytes: dataset.max_persistable_channel_bytes,
    }
}

/// Redaction failure classes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RedactionError {
    #[error("redaction policy violation")]
    PolicyViolation,
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

    /// Detect whether normalized output still contains policy-defined secret patterns.
    #[must_use]
    fn has_policy_residual_leak(&self, output: &PersistableOutput, hints: &RedactionHints) -> bool;
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
            let out = truncate_for_persistence(value, self.policy.max_persistable_channel_bytes);
            let hint_key_names = hints
                .required_secret_names
                .iter()
                .map(SecretName::as_str)
                .map(str::to_owned)
                .collect::<HashSet<_>>();

            let mut ranges = scanner::collect_assignment_value_ranges(
                &out,
                &self.normalized_sensitive_key_names,
                &hint_key_names,
                REDACTION_TOKEN,
            );
            ranges.extend(scanner::collect_bearer_token_ranges(
                &out,
                self.policy.min_token_len,
                REDACTION_TOKEN,
            ));
            ranges.extend(scanner::collect_prefixed_token_ranges(
                &out,
                &self.policy.token_prefixes,
                self.policy.min_token_len,
                REDACTION_TOKEN,
            ));
            ranges.extend(collect_explicit_secret_ranges(
                &out,
                hints,
                self.policy.min_secret_fragment_len,
            ));

            apply_redaction_ranges(out, ranges)
        })
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

    fn channels(output: &PersistableOutput) -> [Option<&str>; 3] {
        [
            output.gate_output.as_deref(),
            output.tail_output.as_deref(),
            output.stderr_tail.as_deref(),
        ]
    }
}

impl OutputRedactor for DefaultOutputRedactor {
    fn redact(
        &self,
        output: RawExecutionOutput,
        hints: &RedactionHints,
    ) -> Result<PersistableOutput, RedactionError> {
        Ok(PersistableOutput {
            outcome: output.outcome,
            signal: output.signal,
            exit_code: output.exit_code,
            duration_secs: output.duration_secs,
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

    fn has_policy_residual_leak(&self, output: &PersistableOutput, hints: &RedactionHints) -> bool {
        let hint_keys = hints
            .required_secret_names
            .iter()
            .map(SecretName::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>();

        for channel in Self::channels(output).iter().flatten() {
            for key in &self.policy.sensitive_key_names {
                if scanner::contains_unredacted_assignment(channel, key, REDACTION_TOKEN) {
                    return true;
                }
            }
            for key in &hint_keys {
                if scanner::contains_unredacted_assignment(channel, key, REDACTION_TOKEN) {
                    return true;
                }
            }
            if scanner::contains_unredacted_bearer_token(
                channel,
                self.policy.min_token_len,
                REDACTION_TOKEN,
            ) {
                return true;
            }
            for prefix in &self.policy.token_prefixes {
                if scanner::contains_unredacted_prefixed_token(
                    channel,
                    prefix,
                    self.policy.min_token_len,
                    REDACTION_TOKEN,
                ) {
                    return true;
                }
            }
        }
        false
    }
}

fn collect_explicit_secret_ranges(
    text: &str,
    hints: &RedactionHints,
    min_secret_fragment_len: usize,
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for secret in &hints.secret_values {
        let value = secret.expose();
        if value.trim().is_empty() {
            continue;
        }
        ranges.extend(collect_literal_ranges(text, value));
        if value.contains('\n') {
            for fragment in value.lines() {
                let trimmed = fragment.trim();
                if trimmed.len() >= min_secret_fragment_len {
                    ranges.extend(collect_literal_ranges(text, trimmed));
                }
            }
        }
    }
    ranges
}

fn collect_literal_ranges(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    let mut ranges = Vec::new();
    let mut search_from = 0;
    while let Some(found) = haystack[search_from..].find(needle) {
        let start = search_from + found;
        let end = start + needle.len();
        ranges.push((start, end));
        search_from = end;
    }
    ranges
}

fn apply_redaction_ranges(mut text: String, mut ranges: Vec<(usize, usize)>) -> String {
    if ranges.is_empty() {
        return text;
    }

    ranges.sort_unstable_by_key(|(start, _)| *start);
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some((_, prev_end)) = merged.last_mut()
            && start <= *prev_end
        {
            *prev_end = (*prev_end).max(end);
            continue;
        }
        merged.push((start, end));
    }

    for (start, end) in merged.into_iter().rev() {
        if start < end && end <= text.len() {
            text.replace_range(start..end, REDACTION_TOKEN);
        }
    }
    text
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
