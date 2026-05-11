//! Configuration and Secrets subsystem.
//!
//! Configuration is tier-scoped (user, account, project, organization) with
//! deterministic inheritance. Secret values are encrypted at rest and never
//! recorded in event payloads, projection files, or proof artifacts; only
//! non-secret metadata is event-replayable.
//!
//! Credential sealing uses an installation-unique salt persisted in the
//! `store_metadata` table. All RNG calls in the sealing path use
//! `getrandom::fill` (fallible OS CSPRNG) rather than `rand::random`
//! (which panics on entropy exhaustion).

pub mod passphrase;
pub mod secret_store;

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use passphrase::CredentialSealPassphrase;
pub use secret_store::{
    INSTALLATION_SEAL_SALT_LEN, InstallationSealSalt, METADATA_KEY_INSTALLATION_SEAL_SALT,
    seal_sync, unseal_sync,
};

/// Configuration tiers in inheritance order, from most-specific to most-general.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Tier {
    /// User-tier configuration. Most specific.
    User,
    /// Project-tier configuration.
    Project,
    /// Account-tier configuration.
    Account,
    /// Organization-tier configuration. Most general.
    Organization,
}

/// Replayable metadata for a stored secret. The secret value itself is held
/// out-of-band by an encrypted store and never appears in this record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// Stable secret identifier (slug, not value).
    pub id: String,
    /// Owning configuration tier.
    pub tier: Tier,
    /// Provider or harness this secret is associated with, if any.
    pub provider: Option<String>,
    /// True once a value has been written; false until the first set.
    pub present: bool,
}

/// Holder for a freshly-resolved secret value. The wrapper zeroes on drop.
#[derive(Debug, Clone)]
pub struct ResolvedSecret {
    /// Identifier this value resolved against.
    pub id: String,
    /// The secret value. Zeroed on drop via [`secrecy::SecretString`].
    pub value: SecretString,
}

/// Errors raised by configuration and secrets operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigSecretsError {
    /// Lookup found no value at any tier.
    #[error("no value found for key '{0}'")]
    NotFound(String),
    /// The operating system CSPRNG could not provide entropy. Indicates a
    /// degraded runtime (e.g. missing `/dev/urandom`, exhausted entropy pool,
    /// or sandbox `getrandom` unavailable).
    #[error("OS entropy source unavailable")]
    EntropySourceUnavailable,
    /// A sealed envelope could not be decoded (malformed base64, truncated
    /// nonce, or invalid UTF-8 after decryption).
    #[error("failed to decode sealed credential")]
    SealDecodeFailed,
    /// The supplied passphrase did not meet the minimum entropy threshold
    /// as measured by the zxcvbn strength estimator. Patterned strings
    /// like "aaaaaaaaaaaa" or "passwordpassword" trigger this error.
    #[error("passphrase does not meet minimum entropy requirements")]
    PassphraseTooWeak,
    /// The zxcvbn estimator could not evaluate the passphrase (e.g. empty
    /// string or internal failure).
    #[error("passphrase strength validation failed")]
    PassphraseValidationFailed,
}
