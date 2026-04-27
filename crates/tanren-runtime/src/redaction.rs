use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, RwLock};

use crate::execution::{PersistableOutput, RawExecutionOutput, RedactionSecret, SecretName};

mod compiled_hints;
mod hints;
pub(crate) mod policy;
pub(crate) mod policy_dataset;
pub(crate) mod scanner;
pub(crate) mod secret_matcher;

use self::compiled_hints::CompiledHintArtifacts;
pub use self::hints::{
    MAX_REDACTION_HINT_SECRET_BYTES, MAX_REDACTION_HINT_SECRET_COUNT,
    MAX_REDACTION_HINT_TOTAL_SECRET_BYTES, RedactionHintBoundsError,
};
pub use self::policy::{
    RedactionPolicy, RedactionPolicyBuilder, RedactionPolicyError, default_redaction_policy,
    default_redaction_policy_dataset_version,
};
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

/// Redaction failure classes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RedactionError {
    #[error(transparent)]
    InvalidHints(#[from] RedactionHintBoundsError),
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
    token_prefix_matcher: scanner::CompiledTokenPrefixMatcher,
    cached_hint_artifacts: Arc<RwLock<Option<CompiledHintArtifacts>>>,
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
            .sensitive_key_names()
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect();
        let token_prefix_matcher =
            scanner::CompiledTokenPrefixMatcher::new(policy.token_prefixes());
        Self {
            policy,
            normalized_sensitive_key_names,
            token_prefix_matcher,
            cached_hint_artifacts: Arc::new(RwLock::new(None)),
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
        let explicit_secret_ranges = explicit_secret_matcher.collect_ranges(&value);
        let mut ranges = Vec::with_capacity(
            artifacts.assignment_value_ranges.len()
                + artifacts.bearer_token_ranges.len()
                + artifacts.prefixed_token_ranges.len()
                + explicit_secret_ranges.len(),
        );
        ranges.extend(artifacts.assignment_value_ranges.iter().copied());
        ranges.extend(artifacts.bearer_token_ranges.iter().copied());
        ranges.extend(artifacts.prefixed_token_ranges.iter().copied());
        ranges.extend(explicit_secret_ranges);

        let redaction_result = apply_redaction_ranges(value, ranges);
        let redacted = truncate_for_persistence(
            redaction_result.redacted,
            self.policy.max_persistable_channel_bytes(),
        );
        let requires_fallback_verification = redaction_result.had_invalid_ranges;
        let known_secret_leak = requires_fallback_verification
            && !explicit_secret_matcher.collect_ranges(&redacted).is_empty();
        let policy_residual_leak = requires_fallback_verification
            && channel_matcher.scan(&redacted).has_policy_residual_leak();

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

#[derive(Debug, Clone, Default)]
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
        if hints.validate_bounds().is_err() {
            return true;
        }
        let compiled_hints = self.compiled_hint_artifacts(hints);
        Self::channels(output).iter().flatten().any(|channel| {
            !compiled_hints
                .secret_matcher
                .collect_ranges(channel)
                .is_empty()
        })
    }

    fn has_policy_residual_leak(&self, output: &PersistableOutput, hints: &RedactionHints) -> bool {
        if hints.validate_bounds().is_err() {
            return true;
        }
        let compiled_hints = self.compiled_hint_artifacts(hints);
        let matcher = CompiledChannelMatcher::new(
            &self.normalized_sensitive_key_names,
            compiled_hints.hint_keys.as_ref(),
            &self.token_prefix_matcher,
            self.policy.min_token_len(),
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
        hints.validate_bounds()?;
        let RawExecutionOutput {
            outcome,
            signal,
            exit_code,
            duration_secs,
            gate_output,
            tail_output,
            stderr_tail,
            pushed,
            plan_hash,
            unchecked_tasks,
            spec_modified,
            findings,
            token_usage,
        } = output;
        let gate_tail_same = gate_output.as_deref() == tail_output.as_deref();
        let gate_stderr_same = gate_output.as_deref() == stderr_tail.as_deref();
        let tail_stderr_same = tail_output.as_deref() == stderr_tail.as_deref();

        let compiled_hints = self.compiled_hint_artifacts(hints);
        let channel_matcher = CompiledChannelMatcher::new(
            &self.normalized_sensitive_key_names,
            compiled_hints.hint_keys.as_ref(),
            &self.token_prefix_matcher,
            self.policy.min_token_len(),
            REDACTION_TOKEN,
        );

        let gate = self.redact_channel(
            gate_output,
            &channel_matcher,
            compiled_hints.secret_matcher.as_ref(),
        );
        let tail = if gate_tail_same {
            gate.clone()
        } else {
            self.redact_channel(
                tail_output,
                &channel_matcher,
                compiled_hints.secret_matcher.as_ref(),
            )
        };
        let stderr = if gate_stderr_same {
            gate.clone()
        } else if tail_stderr_same {
            tail.clone()
        } else {
            self.redact_channel(
                stderr_tail,
                &channel_matcher,
                compiled_hints.secret_matcher.as_ref(),
            )
        };

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
                outcome,
                signal,
                exit_code,
                duration_secs,
                gate_output: gate.value,
                tail_output: tail.value,
                stderr_tail: stderr.value,
                pushed,
                plan_hash,
                unchecked_tasks,
                spec_modified,
                findings,
                token_usage,
            },
            audit,
        })
    }
}

struct CompiledChannelMatcher<'a> {
    policy_keys: &'a HashSet<String>,
    hint_keys: &'a HashSet<String>,
    token_prefix_matcher: &'a scanner::CompiledTokenPrefixMatcher,
    min_token_len: usize,
    redaction_token: &'a str,
}

impl<'a> CompiledChannelMatcher<'a> {
    fn new(
        policy_keys: &'a HashSet<String>,
        hint_keys: &'a HashSet<String>,
        token_prefix_matcher: &'a scanner::CompiledTokenPrefixMatcher,
        min_token_len: usize,
        redaction_token: &'a str,
    ) -> Self {
        Self {
            policy_keys,
            hint_keys,
            token_prefix_matcher,
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
                token_prefix_matcher: self.token_prefix_matcher,
                min_token_len: self.min_token_len,
                redaction_token: self.redaction_token,
            },
        )
    }
}

struct RedactionApplyResult {
    redacted: String,
    had_invalid_ranges: bool,
}

fn apply_redaction_ranges(text: String, mut ranges: Vec<(usize, usize)>) -> RedactionApplyResult {
    if ranges.is_empty() {
        return RedactionApplyResult {
            redacted: text,
            had_invalid_ranges: false,
        };
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
    let mut had_invalid_ranges = false;
    let mut cursor = 0;
    for (start, end) in merged {
        if !(start < end
            && end <= text.len()
            && text.is_char_boundary(start)
            && text.is_char_boundary(end))
        {
            had_invalid_ranges = true;
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
    RedactionApplyResult {
        redacted,
        had_invalid_ranges,
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
