use crate::execution::RedactionSecret;

use super::RedactionHints;

pub const MAX_REDACTION_HINT_SECRET_COUNT: usize = 256;
pub const MAX_REDACTION_HINT_SECRET_BYTES: usize = 4096;
pub const MAX_REDACTION_HINT_TOTAL_SECRET_BYTES: usize = 65_536;

impl RedactionHints {
    /// Build redaction hints from request secret data with hard safety bounds.
    ///
    /// # Errors
    /// Returns [`RedactionHintBoundsError`] when count or size limits are exceeded.
    pub fn try_from_request(
        required_secret_names: Vec<crate::execution::SecretName>,
        secret_values_for_redaction: &[RedactionSecret],
    ) -> Result<Self, RedactionHintBoundsError> {
        let secret_values =
            Self::normalize_and_validate_secret_values(secret_values_for_redaction)?;
        Ok(Self {
            required_secret_names,
            secret_values,
        })
    }

    /// Validate existing hints against hard safety bounds.
    ///
    /// # Errors
    /// Returns [`RedactionHintBoundsError`] when count or size limits are exceeded.
    pub fn validate_bounds(&self) -> Result<(), RedactionHintBoundsError> {
        Self::validate_secret_values(&self.secret_values)
    }

    fn normalize_and_validate_secret_values(
        secret_values_for_redaction: &[RedactionSecret],
    ) -> Result<Vec<RedactionSecret>, RedactionHintBoundsError> {
        if secret_values_for_redaction.len() > MAX_REDACTION_HINT_SECRET_COUNT {
            return Err(RedactionHintBoundsError::TooManySecrets {
                max_count: MAX_REDACTION_HINT_SECRET_COUNT,
                actual_count: secret_values_for_redaction.len(),
            });
        }

        let mut normalized = Vec::with_capacity(secret_values_for_redaction.len());
        let mut total_bytes = 0_usize;
        for (index, secret) in secret_values_for_redaction.iter().enumerate() {
            let trimmed = secret.expose().trim();
            if trimmed.is_empty() {
                continue;
            }

            let secret_bytes = trimmed.len();
            if secret_bytes > MAX_REDACTION_HINT_SECRET_BYTES {
                return Err(RedactionHintBoundsError::SecretTooLarge {
                    index,
                    max_bytes: MAX_REDACTION_HINT_SECRET_BYTES,
                    actual_bytes: secret_bytes,
                });
            }

            total_bytes += secret_bytes;
            if total_bytes > MAX_REDACTION_HINT_TOTAL_SECRET_BYTES {
                return Err(RedactionHintBoundsError::TotalBytesExceeded {
                    max_total_bytes: MAX_REDACTION_HINT_TOTAL_SECRET_BYTES,
                    actual_total_bytes: total_bytes,
                });
            }

            normalized.push(RedactionSecret::from(trimmed));
        }

        Ok(normalized)
    }

    fn validate_secret_values(
        secret_values: &[RedactionSecret],
    ) -> Result<(), RedactionHintBoundsError> {
        if secret_values.len() > MAX_REDACTION_HINT_SECRET_COUNT {
            return Err(RedactionHintBoundsError::TooManySecrets {
                max_count: MAX_REDACTION_HINT_SECRET_COUNT,
                actual_count: secret_values.len(),
            });
        }

        let mut total_bytes = 0_usize;
        for (index, secret) in secret_values.iter().enumerate() {
            let trimmed = secret.expose().trim();
            if trimmed.is_empty() {
                continue;
            }

            let secret_bytes = trimmed.len();
            if secret_bytes > MAX_REDACTION_HINT_SECRET_BYTES {
                return Err(RedactionHintBoundsError::SecretTooLarge {
                    index,
                    max_bytes: MAX_REDACTION_HINT_SECRET_BYTES,
                    actual_bytes: secret_bytes,
                });
            }
            total_bytes += secret_bytes;
            if total_bytes > MAX_REDACTION_HINT_TOTAL_SECRET_BYTES {
                return Err(RedactionHintBoundsError::TotalBytesExceeded {
                    max_total_bytes: MAX_REDACTION_HINT_TOTAL_SECRET_BYTES,
                    actual_total_bytes: total_bytes,
                });
            }
        }
        Ok(())
    }
}

/// Hard bounds errors for explicit redaction hint input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RedactionHintBoundsError {
    #[error("too many explicit redaction secrets: {actual_count} > {max_count}")]
    TooManySecrets {
        max_count: usize,
        actual_count: usize,
    },
    #[error("explicit redaction secret {index} exceeds byte limit: {actual_bytes} > {max_bytes}")]
    SecretTooLarge {
        index: usize,
        max_bytes: usize,
        actual_bytes: usize,
    },
    #[error(
        "total explicit redaction secret bytes exceed limit: {actual_total_bytes} > {max_total_bytes}"
    )]
    TotalBytesExceeded {
        max_total_bytes: usize,
        actual_total_bytes: usize,
    },
}
