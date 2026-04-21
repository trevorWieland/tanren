use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::execution::{PersistableOutput, RawExecutionOutput, RedactionSecret, SecretName};

pub(crate) mod policy_dataset;
pub(crate) mod scanner;
pub(crate) mod secret_matcher;

use self::secret_matcher::CompiledSecretMatcher;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionAudit {
    pub known_secret_leak: bool,
    pub policy_residual_leak: bool,
}

impl RedactionAudit {
    #[must_use]
    pub const fn has_any_leak(&self) -> bool {
        self.known_secret_leak || self.policy_residual_leak
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedactionOutcome {
    pub output: PersistableOutput,
    pub audit: RedactionAudit,
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

    /// Apply redaction and return an audit verdict in one call.
    ///
    /// Default implementation preserves compatibility for custom redactors that
    /// only implement `redact` and leak-detection hooks.
    ///
    /// # Errors
    /// Returns [`RedactionError`] if output cannot be normalized safely.
    fn redact_with_audit(
        &self,
        output: RawExecutionOutput,
        hints: &RedactionHints,
    ) -> Result<RedactionOutcome, RedactionError> {
        let output = self.redact(output, hints)?;
        let audit = RedactionAudit {
            known_secret_leak: self.has_known_secret_leak(&output, hints),
            policy_residual_leak: self.has_policy_residual_leak(&output, hints),
        };
        Ok(RedactionOutcome { output, audit })
    }
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

    fn redact_channel(
        &self,
        input: Option<String>,
        channel_matcher: &CompiledChannelMatcher<'_>,
        explicit_secret_matcher: &CompiledSecretMatcher,
    ) -> ChannelRedactionResult {
        let Some(value) = input else {
            return ChannelRedactionResult::default();
        };
        let artifacts = channel_matcher.scan(&value);
        let mut ranges = artifacts.assignment_value_ranges.clone();
        ranges.extend_from_slice(&artifacts.bearer_token_ranges);
        ranges.extend_from_slice(&artifacts.prefixed_token_ranges);
        ranges.extend(explicit_secret_matcher.collect_ranges(&value));
        let redacted = apply_redaction_ranges(value, ranges);
        let redacted =
            truncate_for_persistence(redacted, self.policy.max_persistable_channel_bytes);
        let post_scan = channel_matcher.scan(&redacted);
        let known_secret_leak = !explicit_secret_matcher.collect_ranges(&redacted).is_empty();
        let policy_residual_leak = post_scan.has_policy_residual_leak();

        ChannelRedactionResult {
            value: Some(redacted),
            known_secret_leak,
            policy_residual_leak,
        }
    }

    fn channels(output: &PersistableOutput) -> [Option<&str>; 3] {
        [
            output.gate_output.as_deref(),
            output.tail_output.as_deref(),
            output.stderr_tail.as_deref(),
        ]
    }
}

#[derive(Debug, Default)]
struct ChannelRedactionResult {
    value: Option<String>,
    known_secret_leak: bool,
    policy_residual_leak: bool,
}

impl OutputRedactor for DefaultOutputRedactor {
    fn redact(
        &self,
        output: RawExecutionOutput,
        hints: &RedactionHints,
    ) -> Result<PersistableOutput, RedactionError> {
        self.redact_with_audit(output, hints)
            .map(|result| result.output)
    }

    fn has_known_secret_leak(&self, output: &PersistableOutput, hints: &RedactionHints) -> bool {
        let matcher = CompiledSecretMatcher::from_hints(hints, self.policy.min_secret_fragment_len);
        Self::channels(output)
            .iter()
            .flatten()
            .any(|channel| !matcher.collect_ranges(channel).is_empty())
    }

    fn has_policy_residual_leak(&self, output: &PersistableOutput, hints: &RedactionHints) -> bool {
        let hint_keys = hints
            .required_secret_names
            .iter()
            .map(SecretName::as_str)
            .map(str::to_owned)
            .collect::<HashSet<_>>();
        let matcher = CompiledChannelMatcher::new(
            &self.normalized_sensitive_key_names,
            &hint_keys,
            &self.policy.token_prefixes,
            self.policy.min_token_len,
            REDACTION_TOKEN,
        );

        for channel in Self::channels(output).iter().flatten() {
            if matcher.scan(channel).has_policy_residual_leak() {
                return true;
            }
        }
        false
    }

    fn redact_with_audit(
        &self,
        output: RawExecutionOutput,
        hints: &RedactionHints,
    ) -> Result<RedactionOutcome, RedactionError> {
        let hint_keys = hints
            .required_secret_names
            .iter()
            .map(SecretName::as_str)
            .map(str::to_owned)
            .collect::<HashSet<_>>();
        let channel_matcher = CompiledChannelMatcher::new(
            &self.normalized_sensitive_key_names,
            &hint_keys,
            &self.policy.token_prefixes,
            self.policy.min_token_len,
            REDACTION_TOKEN,
        );
        let explicit_secret_matcher =
            CompiledSecretMatcher::from_hints(hints, self.policy.min_secret_fragment_len);

        let gate = self.redact_channel(
            output.gate_output,
            &channel_matcher,
            &explicit_secret_matcher,
        );
        let tail = self.redact_channel(
            output.tail_output,
            &channel_matcher,
            &explicit_secret_matcher,
        );
        let stderr = self.redact_channel(
            output.stderr_tail,
            &channel_matcher,
            &explicit_secret_matcher,
        );

        let audit = RedactionAudit {
            known_secret_leak: gate.known_secret_leak
                || tail.known_secret_leak
                || stderr.known_secret_leak,
            policy_residual_leak: gate.policy_residual_leak
                || tail.policy_residual_leak
                || stderr.policy_residual_leak,
        };

        Ok(RedactionOutcome {
            output: PersistableOutput {
                outcome: output.outcome,
                signal: output.signal,
                exit_code: output.exit_code,
                duration_secs: output.duration_secs,
                gate_output: gate.value,
                tail_output: tail.value,
                stderr_tail: stderr.value,
                pushed: output.pushed,
                plan_hash: output.plan_hash,
                unchecked_tasks: output.unchecked_tasks,
                spec_modified: output.spec_modified,
                findings: output.findings,
                token_usage: output.token_usage,
            },
            audit,
        })
    }
}

struct CompiledChannelMatcher<'a> {
    policy_keys: &'a HashSet<String>,
    hint_keys: &'a HashSet<String>,
    token_prefixes: &'a [String],
    min_token_len: usize,
    redaction_token: &'a str,
}

impl<'a> CompiledChannelMatcher<'a> {
    fn new(
        policy_keys: &'a HashSet<String>,
        hint_keys: &'a HashSet<String>,
        token_prefixes: &'a [String],
        min_token_len: usize,
        redaction_token: &'a str,
    ) -> Self {
        Self {
            policy_keys,
            hint_keys,
            token_prefixes,
            min_token_len,
            redaction_token,
        }
    }

    fn scan(&self, text: &str) -> scanner::ChannelScanArtifacts {
        scanner::scan_channel(
            text,
            &scanner::ChannelScanConfig {
                policy_keys: self.policy_keys,
                hint_keys: self.hint_keys,
                token_prefixes: self.token_prefixes,
                min_token_len: self.min_token_len,
                redaction_token: self.redaction_token,
            },
        )
    }
}

fn apply_redaction_ranges(text: String, mut ranges: Vec<(usize, usize)>) -> String {
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

    let mut redacted = String::with_capacity(text.len());
    let mut cursor = 0;
    for (start, end) in merged {
        if !(start < end
            && end <= text.len()
            && text.is_char_boundary(start)
            && text.is_char_boundary(end))
        {
            continue;
        }

        if cursor < start {
            redacted.push_str(&text[cursor..start]);
        }
        redacted.push_str(REDACTION_TOKEN);
        cursor = end;
    }
    if cursor < text.len() {
        redacted.push_str(&text[cursor..]);
    }
    redacted
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
