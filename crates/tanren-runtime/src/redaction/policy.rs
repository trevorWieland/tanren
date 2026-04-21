use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::policy_dataset;

const MIN_TOKEN_LEN_LOWER_BOUND: usize = 6;
const MIN_SECRET_FRAGMENT_LEN_LOWER_BOUND: usize = 3;
const MIN_PERSISTABLE_CHANNEL_BYTES: usize = 1024;

/// Deterministic redaction policy used by all adapters unless overridden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RedactionPolicy {
    min_token_len: usize,
    min_secret_fragment_len: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sensitive_key_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    token_prefixes: Vec<String>,
    max_persistable_channel_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RedactionPolicyError {
    #[error("min_token_len must be at least {minimum}")]
    MinTokenLenTooSmall { minimum: usize },
    #[error(
        "min_secret_fragment_len must be between {minimum} and min_token_len ({actual_min_token_len})"
    )]
    MinSecretFragmentLenOutOfRange {
        minimum: usize,
        actual_min_token_len: usize,
    },
    #[error("max_persistable_channel_bytes must be at least {minimum}")]
    MaxPersistableChannelBytesTooSmall { minimum: usize },
    #[error("sensitive key names must not contain empty entries")]
    EmptySensitiveKeyName,
    #[error("token prefixes must not contain empty entries")]
    EmptyTokenPrefix,
    #[error("duplicate sensitive key name `{name}`")]
    DuplicateSensitiveKeyName { name: String },
    #[error("duplicate token prefix `{prefix}`")]
    DuplicateTokenPrefix { prefix: String },
}

impl RedactionPolicy {
    pub fn try_new(
        min_token_len: usize,
        min_secret_fragment_len: usize,
        sensitive_key_names: Vec<String>,
        token_prefixes: Vec<String>,
        max_persistable_channel_bytes: usize,
    ) -> Result<Self, RedactionPolicyError> {
        if min_token_len < MIN_TOKEN_LEN_LOWER_BOUND {
            return Err(RedactionPolicyError::MinTokenLenTooSmall {
                minimum: MIN_TOKEN_LEN_LOWER_BOUND,
            });
        }
        if !(MIN_SECRET_FRAGMENT_LEN_LOWER_BOUND..=min_token_len).contains(&min_secret_fragment_len)
        {
            return Err(RedactionPolicyError::MinSecretFragmentLenOutOfRange {
                minimum: MIN_SECRET_FRAGMENT_LEN_LOWER_BOUND,
                actual_min_token_len: min_token_len,
            });
        }
        if max_persistable_channel_bytes < MIN_PERSISTABLE_CHANNEL_BYTES {
            return Err(RedactionPolicyError::MaxPersistableChannelBytesTooSmall {
                minimum: MIN_PERSISTABLE_CHANNEL_BYTES,
            });
        }

        let normalized_sensitive_key_names = normalize_non_empty_lowercase(
            sensitive_key_names,
            || RedactionPolicyError::EmptySensitiveKeyName,
            |name| RedactionPolicyError::DuplicateSensitiveKeyName { name },
        )?;
        let normalized_token_prefixes = normalize_non_empty_lowercase(
            token_prefixes,
            || RedactionPolicyError::EmptyTokenPrefix,
            |prefix| RedactionPolicyError::DuplicateTokenPrefix { prefix },
        )?;

        Ok(Self {
            min_token_len,
            min_secret_fragment_len,
            sensitive_key_names: normalized_sensitive_key_names,
            token_prefixes: normalized_token_prefixes,
            max_persistable_channel_bytes,
        })
    }

    #[must_use]
    pub fn builder() -> RedactionPolicyBuilder {
        RedactionPolicyBuilder::default()
    }

    #[must_use]
    pub const fn min_token_len(&self) -> usize {
        self.min_token_len
    }

    #[must_use]
    pub const fn min_secret_fragment_len(&self) -> usize {
        self.min_secret_fragment_len
    }

    #[must_use]
    pub fn sensitive_key_names(&self) -> &[String] {
        &self.sensitive_key_names
    }

    #[must_use]
    pub fn token_prefixes(&self) -> &[String] {
        &self.token_prefixes
    }

    #[must_use]
    pub const fn max_persistable_channel_bytes(&self) -> usize {
        self.max_persistable_channel_bytes
    }
}

#[derive(Debug, Clone)]
pub struct RedactionPolicyBuilder {
    min_token_len: usize,
    min_secret_fragment_len: usize,
    sensitive_key_names: Vec<String>,
    token_prefixes: Vec<String>,
    max_persistable_channel_bytes: usize,
}

impl Default for RedactionPolicyBuilder {
    fn default() -> Self {
        let dataset = policy_dataset::default_policy_dataset_v1();
        Self {
            min_token_len: dataset.min_token_len,
            min_secret_fragment_len: dataset.min_secret_fragment_len,
            sensitive_key_names: dataset
                .sensitive_key_names
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
            token_prefixes: dataset
                .token_prefixes
                .iter()
                .map(|prefix| (*prefix).to_owned())
                .collect(),
            max_persistable_channel_bytes: dataset.max_persistable_channel_bytes,
        }
    }
}

impl RedactionPolicyBuilder {
    #[must_use]
    pub fn min_token_len(mut self, value: usize) -> Self {
        self.min_token_len = value;
        self
    }

    #[must_use]
    pub fn min_secret_fragment_len(mut self, value: usize) -> Self {
        self.min_secret_fragment_len = value;
        self
    }

    #[must_use]
    pub fn sensitive_key_names(mut self, value: Vec<String>) -> Self {
        self.sensitive_key_names = value;
        self
    }

    #[must_use]
    pub fn token_prefixes(mut self, value: Vec<String>) -> Self {
        self.token_prefixes = value;
        self
    }

    #[must_use]
    pub fn max_persistable_channel_bytes(mut self, value: usize) -> Self {
        self.max_persistable_channel_bytes = value;
        self
    }

    pub fn build(self) -> Result<RedactionPolicy, RedactionPolicyError> {
        RedactionPolicy::try_new(
            self.min_token_len,
            self.min_secret_fragment_len,
            self.sensitive_key_names,
            self.token_prefixes,
            self.max_persistable_channel_bytes,
        )
    }
}

#[derive(Deserialize)]
struct RedactionPolicyWire {
    min_token_len: usize,
    min_secret_fragment_len: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sensitive_key_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    token_prefixes: Vec<String>,
    max_persistable_channel_bytes: usize,
}

impl<'de> Deserialize<'de> for RedactionPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RedactionPolicyWire::deserialize(deserializer)?;
        Self::try_new(
            wire.min_token_len,
            wire.min_secret_fragment_len,
            wire.sensitive_key_names,
            wire.token_prefixes,
            wire.max_persistable_channel_bytes,
        )
        .map_err(serde::de::Error::custom)
    }
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
            .map(|value| (*value).to_ascii_lowercase())
            .collect(),
        token_prefixes: dataset
            .token_prefixes
            .iter()
            .map(|value| (*value).to_ascii_lowercase())
            .collect(),
        max_persistable_channel_bytes: dataset.max_persistable_channel_bytes,
    }
}

fn normalize_non_empty_lowercase(
    values: Vec<String>,
    empty_err: impl Fn() -> RedactionPolicyError,
    duplicate_err: impl Fn(String) -> RedactionPolicyError,
) -> Result<Vec<String>, RedactionPolicyError> {
    let mut normalized = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for value in values {
        let normalized_value = value.trim().to_ascii_lowercase();
        if normalized_value.is_empty() {
            return Err(empty_err());
        }
        if !seen.insert(normalized_value.clone()) {
            return Err(duplicate_err(normalized_value));
        }
        normalized.push(normalized_value);
    }
    Ok(normalized)
}
