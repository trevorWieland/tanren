//! Secret-store adapter contract for user credential value sealing.
//!
//! This module owns the credential sealing policy contract:
//! - key source: `TANREN_CREDENTIAL_SEAL_PASSPHRASE` from process env;
//! - operator passphrase validation: [`CredentialSealPassphrase`];
//! - key derivation: Argon2id (v1 policy constants below);
//! - random per-write KDF salt and nonce generation;
//! - AEAD associated data binding to credential id, owner account/scope,
//!   credential kind, and seal contract version;
//! - typed failures for configuration, KDF, and encryption errors.
//!
//! Rotation boundary: every call to [`seal_user_credential_value_from_env`]
//! mints a fresh random salt + nonce pair and yields one independent sealed
//! payload version for persistence.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use secrecy::{ExposeSecret, SecretString};
use tanren_identity_policy::AccountId;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    CredentialSealPassphrase, CredentialSealPassphraseValidationFailure, OwnerScope,
    UserCredentialId, UserCredentialKind, user_credential_kind_wire_name,
};

/// Environment variable providing the credential-sealing passphrase source.
pub const CREDENTIAL_SEAL_PASSPHRASE_ENV: &str = "TANREN_CREDENTIAL_SEAL_PASSPHRASE";
const KDF_VERSION_V1: i16 = 1;
const KDF_SALT_LEN_BYTES: usize = 16;
const CREDENTIAL_KEY_LEN_BYTES: usize = 32;
const CREDENTIAL_NONCE_LEN_BYTES: usize = 12;
const ARGON2_MEMORY_COST_KIB: u32 = 19_456;
const ARGON2_TIME_COST: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;
const AAD_VERSION_V1: &[u8] = b"tanren.user_credential.seal.v1";
const KDF_VERSION_WIRE_V1: &[u8] = b"1";

/// Inputs bound into credential sealing associated data.
#[derive(Debug, Clone, Copy)]
pub struct UserCredentialSealContext {
    /// Credential metadata id the ciphertext belongs to.
    pub credential_id: UserCredentialId,
    /// Owner scope the ciphertext belongs to.
    pub owner_scope: OwnerScope,
    /// Credential kind the ciphertext belongs to.
    pub credential_kind: UserCredentialKind,
}

/// Supported at-rest cipher policy identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSealScheme {
    /// ChaCha20-Poly1305 AEAD policy, version 1.
    ChaCha20Poly1305V1,
}

impl CredentialSealScheme {
    /// Stable wire/db label for this scheme.
    #[must_use]
    pub const fn as_db_value(self) -> &'static str {
        match self {
            Self::ChaCha20Poly1305V1 => "chacha20poly1305-v1",
        }
    }

    const fn current() -> Self {
        Self::ChaCha20Poly1305V1
    }
}

/// Sealed credential payload ready for persistence.
#[derive(Clone)]
pub struct SealedUserCredentialValue {
    credential_id: UserCredentialId,
    owner_scope: OwnerScope,
    credential_kind: UserCredentialKind,
    scheme: CredentialSealScheme,
    kdf_version: i16,
    kdf_salt: [u8; KDF_SALT_LEN_BYTES],
    nonce: [u8; CREDENTIAL_NONCE_LEN_BYTES],
    ciphertext: Vec<u8>,
}

impl std::fmt::Debug for SealedUserCredentialValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedUserCredentialValue")
            .field("credential_id", &self.credential_id)
            .field("owner_scope", &self.owner_scope)
            .field("credential_kind", &self.credential_kind)
            .field("scheme", &self.scheme)
            .field("kdf_version", &self.kdf_version)
            .field("kdf_salt_len", &self.kdf_salt.len())
            .field("nonce_len", &self.nonce.len())
            .field("ciphertext_len", &self.ciphertext.len())
            .finish()
    }
}

impl SealedUserCredentialValue {
    /// Credential id bound into the sealed associated-data domain.
    #[must_use]
    pub const fn credential_id(&self) -> UserCredentialId {
        self.credential_id
    }

    /// Owner scope bound into the sealed associated-data domain.
    #[must_use]
    pub const fn owner_scope(&self) -> OwnerScope {
        self.owner_scope
    }

    /// Credential kind bound into the sealed associated-data domain.
    #[must_use]
    pub const fn credential_kind(&self) -> UserCredentialKind {
        self.credential_kind
    }

    /// Cipher scheme identifier.
    #[must_use]
    pub const fn scheme(&self) -> CredentialSealScheme {
        self.scheme
    }

    /// Key-derivation policy version.
    #[must_use]
    pub const fn kdf_version(&self) -> i16 {
        self.kdf_version
    }

    /// Random KDF salt used for this ciphertext.
    #[must_use]
    pub const fn kdf_salt(&self) -> &[u8; KDF_SALT_LEN_BYTES] {
        &self.kdf_salt
    }

    /// Random AEAD nonce used for this ciphertext.
    #[must_use]
    pub const fn nonce(&self) -> &[u8; CREDENTIAL_NONCE_LEN_BYTES] {
        &self.nonce
    }

    /// AEAD ciphertext bytes.
    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }
}

/// Typed failures for credential sealing.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CredentialSealingFailure {
    /// Runtime passphrase source env var is missing.
    #[error("missing credential-seal passphrase in `{env_var}`")]
    MissingPassphraseEnv {
        /// Missing environment variable key.
        env_var: &'static str,
    },
    /// Runtime passphrase source env var failed validation.
    #[error("invalid credential-seal passphrase in `{env_var}`: {source}")]
    InvalidPassphraseEnv {
        /// Environment variable key.
        env_var: &'static str,
        /// Passphrase validation failure.
        #[source]
        source: CredentialSealPassphraseValidationFailure,
    },
    /// Credential kind is not in the supported registry.
    #[error("unsupported credential kind for sealing: '{kind}'")]
    UnsupportedCredentialKind {
        /// Unsupported credential kind debug label.
        kind: String,
    },
    /// Argon2 policy parameters failed initialization.
    #[error("invalid credential-seal argon2 parameters: {detail}")]
    InvalidKdfParameters {
        /// Human-readable initialization detail.
        detail: String,
    },
    /// KDF operation failed.
    #[error("credential-seal key derivation failed: {detail}")]
    KeyDerivation {
        /// Human-readable derivation detail.
        detail: String,
    },
    /// Derived key bytes could not initialize the configured cipher.
    #[error("credential-seal cipher key material invalid")]
    InvalidCipherKey,
    /// AEAD encryption operation failed.
    #[error("credential-seal AEAD encryption failed")]
    Encryption,
    /// Sealing task failed to complete.
    #[error("credential-seal task failed")]
    Task,
}

/// Seal one credential value according to Tanren's v1 user-credential policy.
///
/// The operator passphrase source and validation policy are:
/// - env var: [`CREDENTIAL_SEAL_PASSPHRASE_ENV`]
/// - minimum length: [`crate::CREDENTIAL_SEAL_PASSPHRASE_MIN_BYTES`]
/// - minimum entropy estimate:
///   [`crate::CREDENTIAL_SEAL_PASSPHRASE_MIN_ESTIMATED_ENTROPY_BITS`].
///
/// Associated data binds:
/// - contract version (`tanren.user_credential.seal.v1`)
/// - credential id
/// - owner account id
/// - owner scope tag
/// - credential kind
/// - cipher scheme + KDF version
///
/// # Errors
///
/// Returns [`CredentialSealingFailure`] when passphrase configuration is
/// missing/invalid, key derivation fails, or encryption fails.
pub async fn seal_user_credential_value_from_env(
    context: UserCredentialSealContext,
    value: SecretString,
) -> Result<SealedUserCredentialValue, CredentialSealingFailure> {
    tokio::task::spawn_blocking(move || {
        let encryptor = CredentialValueEncryptor::from_env()?;
        encryptor.seal(context, &value)
    })
    .await
    .map_err(|_| CredentialSealingFailure::Task)?
}

struct CredentialValueEncryptor {
    passphrase: CredentialSealPassphrase,
}

impl CredentialValueEncryptor {
    fn from_env() -> Result<Self, CredentialSealingFailure> {
        let passphrase =
            Zeroizing::new(std::env::var(CREDENTIAL_SEAL_PASSPHRASE_ENV).map_err(|_| {
                CredentialSealingFailure::MissingPassphraseEnv {
                    env_var: CREDENTIAL_SEAL_PASSPHRASE_ENV,
                }
            })?);
        let validated = CredentialSealPassphrase::parse(passphrase.as_str()).map_err(|source| {
            CredentialSealingFailure::InvalidPassphraseEnv {
                env_var: CREDENTIAL_SEAL_PASSPHRASE_ENV,
                source,
            }
        })?;
        Ok(Self {
            passphrase: validated,
        })
    }

    fn seal(
        &self,
        context: UserCredentialSealContext,
        value: &SecretString,
    ) -> Result<SealedUserCredentialValue, CredentialSealingFailure> {
        let credential_kind_wire = user_credential_kind_wire_name(context.credential_kind)
            .map_err(|_| CredentialSealingFailure::UnsupportedCredentialKind {
                kind: format!("{:?}", context.credential_kind),
            })?;
        let kdf_salt = rand::random::<[u8; KDF_SALT_LEN_BYTES]>();
        let aad = associated_data(&context, credential_kind_wire);
        let cipher = self.cipher(&aad, &kdf_salt)?;
        let nonce = rand::random::<[u8; CREDENTIAL_NONCE_LEN_BYTES]>();
        let payload = Payload {
            msg: value.expose_secret().as_bytes(),
            aad: &aad,
        };
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), payload)
            .map_err(|_| CredentialSealingFailure::Encryption)?;
        Ok(SealedUserCredentialValue {
            credential_id: context.credential_id,
            owner_scope: context.owner_scope,
            credential_kind: context.credential_kind,
            scheme: CredentialSealScheme::current(),
            kdf_version: KDF_VERSION_V1,
            kdf_salt,
            nonce,
            ciphertext,
        })
    }

    fn cipher(
        &self,
        associated_data: &[u8],
        kdf_salt: &[u8; KDF_SALT_LEN_BYTES],
    ) -> Result<ChaCha20Poly1305, CredentialSealingFailure> {
        let key_material = self.derive_cipher_key(associated_data, kdf_salt)?;
        ChaCha20Poly1305::new_from_slice(&key_material[..])
            .map_err(|_| CredentialSealingFailure::InvalidCipherKey)
    }

    fn derive_cipher_key(
        &self,
        associated_data: &[u8],
        kdf_salt: &[u8; KDF_SALT_LEN_BYTES],
    ) -> Result<Zeroizing<[u8; CREDENTIAL_KEY_LEN_BYTES]>, CredentialSealingFailure> {
        let params = Params::new(
            ARGON2_MEMORY_COST_KIB,
            ARGON2_TIME_COST,
            ARGON2_PARALLELISM,
            Some(CREDENTIAL_KEY_LEN_BYTES),
        )
        .map_err(|err| CredentialSealingFailure::InvalidKdfParameters {
            detail: err.to_string(),
        })?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut output = Zeroizing::new([0_u8; CREDENTIAL_KEY_LEN_BYTES]);
        let mut salt = Zeroizing::new(Vec::with_capacity(kdf_salt.len() + associated_data.len()));
        salt.extend_from_slice(kdf_salt);
        salt.extend_from_slice(associated_data);
        argon2
            .hash_password_into(self.passphrase.as_bytes(), &salt, &mut *output)
            .map_err(|err| CredentialSealingFailure::KeyDerivation {
                detail: err.to_string(),
            })?;
        Ok(output)
    }
}

fn associated_data(
    context: &UserCredentialSealContext,
    credential_kind_wire: &str,
) -> Zeroizing<Vec<u8>> {
    let (owner_scope_wire, owner_account_id) = owner_scope_parts(context.owner_scope);
    let mut aad = Zeroizing::new(Vec::with_capacity(128 + credential_kind_wire.len()));
    aad.extend_from_slice(AAD_VERSION_V1);
    aad.push(0);
    aad.extend_from_slice(context.credential_id.as_uuid().as_bytes());
    aad.push(0);
    aad.extend_from_slice(owner_account_id.as_uuid().as_bytes());
    aad.push(0);
    aad.extend_from_slice(owner_scope_wire.as_bytes());
    aad.push(0);
    aad.extend_from_slice(credential_kind_wire.as_bytes());
    aad.push(0);
    aad.extend_from_slice(CredentialSealScheme::current().as_db_value().as_bytes());
    aad.push(0);
    aad.extend_from_slice(KDF_VERSION_WIRE_V1);
    aad
}

fn owner_scope_parts(owner_scope: OwnerScope) -> (&'static str, AccountId) {
    match owner_scope {
        OwnerScope::User { account_id } => ("user", account_id),
    }
}
