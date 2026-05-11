//! Passphrase strength validation for credential sealing.
//!
//! [`CredentialSealPassphrase`] wraps a user-supplied passphrase that will
//! be used to derive a credential seal key. Construction goes through
//! [`CredentialSealPassphrase::parse`], which rejects weak passphrases
//! using zxcvbn — a vetted entropy-based strength estimator that detects
//! common patterns (repeated characters, dictionary words, keyboard walks,
//! l33t substitutions, etc.) that a naïve unique-byte heuristic would miss.

use crate::ConfigSecretsError;
use secrecy::{ExposeSecret, SecretString};
use zxcvbn::zxcvbn as estimate_strength;

/// Minimum zxcvbn entropy score (0–4 scale) accepted by
/// [`CredentialSealPassphrase::parse`]. Score 3 corresponds to an
/// entropy level that is "safely unguessable" — an attacker would need
/// substantial resources to brute-force. This is a deliberate policy
/// choice rather than an ad-hoc threshold.
const MIN_ZXCVBN_SCORE: zxcvbn::Score = zxcvbn::Score::Three;

/// A passphrase that has passed zxcvbn strength validation. Wraps a
/// [`SecretString`] so the raw value is zeroed on drop.
#[derive(Debug, Clone)]
pub struct CredentialSealPassphrase {
    inner: SecretString,
}

impl CredentialSealPassphrase {
    /// Parse and validate a user-supplied passphrase string.
    ///
    /// The passphrase is checked against zxcvbn's entropy estimator. If
    /// the estimated strength score is below [`MIN_ZXCVBN_SCORE`], the
    /// passphrase is rejected with
    /// [`ConfigSecretsError::PassphraseTooWeak`].
    ///
    /// # Errors
    ///
    /// Returns [`ConfigSecretsError::PassphraseTooWeak`] when zxcvbn
    /// rates the passphrase below the minimum strength threshold — for
    /// example patterned strings like "aaaaaaaaaaaa" or
    /// "passwordpassword".
    ///
    /// Returns [`ConfigSecretsError::PassphraseValidationFailed`] when
    /// the passphrase is empty.
    pub fn parse(raw: SecretString) -> Result<Self, ConfigSecretsError> {
        let password = raw.expose_secret();

        if password.is_empty() {
            return Err(ConfigSecretsError::PassphraseValidationFailed);
        }

        let entropy = estimate_strength(password, &[]);

        if entropy.score() < MIN_ZXCVBN_SCORE {
            return Err(ConfigSecretsError::PassphraseTooWeak);
        }

        Ok(Self { inner: raw })
    }

    /// Consume the validated passphrase and return the inner
    /// [`SecretString`].
    #[must_use]
    pub fn into_inner(self) -> SecretString {
        self.inner
    }
}
