//! Secret-store adapter contract for user credential value sealing.
//!
//! This module owns the credential sealing policy contract:
//! - key source: `TANREN_CREDENTIAL_SEAL_PASSPHRASE` from process env;
//! - operator passphrase validation: [`CredentialSealPassphrase`];
//! - root material derivation: Argon2id (v1 policy constants below), once
//!   per [`CredentialValueSealer`] instance;
//! - random per-write KDF salt and nonce generation;
//! - AEAD associated data binding to credential id, owner account/scope,
//!   credential kind, and seal contract version;
//! - typed failures for configuration, KDF, encryption, and decryption errors.
//!
//! Rotation boundary: every call to [`CredentialValueSealer::seal`] mints a
//! fresh random salt + nonce pair and yields one independent sealed payload
//! version for persistence.

use std::sync::Arc;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use secrecy::{ExposeSecret, SecretString};
use tanren_identity_policy::AccountId;
use thiserror::Error;
use zeroize::Zeroizing;

mod version;
pub use version::CredentialSealVersion;

use crate::{
    CredentialSealPassphrase, CredentialSealPassphraseValidationFailure, OwnerScope,
    UserCredentialId, UserCredentialKind, user_credential_kind_wire_name,
};

/// Environment variable providing the credential-sealing passphrase source.
pub const CREDENTIAL_SEAL_PASSPHRASE_ENV: &str = "TANREN_CREDENTIAL_SEAL_PASSPHRASE";
const KDF_SALT_LEN_BYTES: usize = 16;
const CREDENTIAL_KEY_LEN_BYTES: usize = 32;
const CREDENTIAL_NONCE_LEN_BYTES: usize = 12;
const ARGON2_MEMORY_COST_KIB: u32 = 19_456;
const ARGON2_TIME_COST: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;
const AAD_VERSION_V1: &[u8] = b"tanren.user_credential.seal.v1";
const ROOT_MATERIAL_DOMAIN_V1: &[u8] = b"tanren.user_credential.seal.root.v1";
const ROOT_MATERIAL_SALT_V1: &[u8; KDF_SALT_LEN_BYTES] = b"tnrn.seal.root1!";

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

    /// Seal contract version bound into the payload.
    #[must_use]
    pub const fn seal_version(&self) -> i16 {
        self.kdf_version
    }

    /// Parse the typed seal contract version.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialSealingFailure::UnsupportedSealVersion`] when
    /// the stored version is unknown.
    pub fn version(&self) -> Result<CredentialSealVersion, CredentialSealingFailure> {
        CredentialSealVersion::parse(self.kdf_version)
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

/// Typed failures for credential sealing/opening.
#[derive(Debug, Clone, Error)]
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
    /// Seal payload version is unsupported by this sealer.
    #[error("unsupported credential seal version: {version}")]
    UnsupportedSealVersion {
        /// Unsupported version value.
        version: i16,
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
    /// AEAD decryption operation failed.
    #[error("credential-seal AEAD decryption failed")]
    Decryption,
    /// Sealing task failed to complete.
    #[error("credential-seal task failed")]
    Task,
}

/// Reusable credential sealer initialized once from installation policy.
#[derive(Clone)]
pub struct CredentialValueSealer {
    inner: Arc<CredentialValueSealerInner>,
}

impl std::fmt::Debug for CredentialValueSealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CredentialValueSealer(<redacted>)")
    }
}

#[derive(Clone)]
struct CredentialValueSealerInner {
    root_material: Zeroizing<[u8; CREDENTIAL_KEY_LEN_BYTES]>,
}

impl CredentialValueSealer {
    /// Build a sealer from the process environment.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialSealingFailure`] when the env var is missing,
    /// invalid, or root material derivation fails.
    pub fn from_env() -> Result<Self, CredentialSealingFailure> {
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
        Self::from_passphrase(&validated)
    }

    /// Build a sealer from a validated passphrase.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialSealingFailure`] when root-material derivation fails.
    pub fn from_passphrase(
        passphrase: &CredentialSealPassphrase,
    ) -> Result<Self, CredentialSealingFailure> {
        let root_material = derive_root_material(passphrase.as_bytes())?;
        Ok(Self {
            inner: Arc::new(CredentialValueSealerInner { root_material }),
        })
    }

    /// Seal one credential value.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialSealingFailure`] when unsupported kind, key
    /// derivation, or AEAD encryption fails.
    pub async fn seal(
        &self,
        context: UserCredentialSealContext,
        value: SecretString,
    ) -> Result<SealedUserCredentialValue, CredentialSealingFailure> {
        let sealer = self.clone();
        tokio::task::spawn_blocking(move || sealer.seal_sync(context, &value))
            .await
            .map_err(|_| CredentialSealingFailure::Task)?
    }

    /// Open one sealed credential value for the provided context.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialSealingFailure`] when the version is unsupported,
    /// kind lookup fails, key derivation fails, or AEAD decryption fails.
    pub async fn open(
        &self,
        value: &SealedUserCredentialValue,
    ) -> Result<SecretString, CredentialSealingFailure> {
        let sealer = self.clone();
        let sealed_value = value.clone();
        tokio::task::spawn_blocking(move || sealer.open_sync(&sealed_value))
            .await
            .map_err(|_| CredentialSealingFailure::Task)?
    }

    fn seal_sync(
        &self,
        context: UserCredentialSealContext,
        value: &SecretString,
    ) -> Result<SealedUserCredentialValue, CredentialSealingFailure> {
        let credential_kind_wire = user_credential_kind_wire_name(context.credential_kind)
            .map_err(|_| CredentialSealingFailure::UnsupportedCredentialKind {
                kind: format!("{:?}", context.credential_kind),
            })?;

        let seal_version = CredentialSealVersion::CURRENT;
        let kdf_salt = rand::random::<[u8; KDF_SALT_LEN_BYTES]>();
        let nonce = rand::random::<[u8; CREDENTIAL_NONCE_LEN_BYTES]>();
        let aad = associated_data(&context, credential_kind_wire, seal_version);
        let cipher = self.cipher(&aad, &kdf_salt, &nonce)?;
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
            kdf_version: seal_version.as_i16(),
            kdf_salt,
            nonce,
            ciphertext,
        })
    }

    fn open_sync(
        &self,
        value: &SealedUserCredentialValue,
    ) -> Result<SecretString, CredentialSealingFailure> {
        let credential_kind_wire =
            user_credential_kind_wire_name(value.credential_kind).map_err(|_| {
                CredentialSealingFailure::UnsupportedCredentialKind {
                    kind: format!("{:?}", value.credential_kind),
                }
            })?;
        let context = UserCredentialSealContext {
            credential_id: value.credential_id,
            owner_scope: value.owner_scope,
            credential_kind: value.credential_kind,
        };
        let seal_version = CredentialSealVersion::parse(value.kdf_version)?;
        let aad = associated_data(&context, credential_kind_wire, seal_version);
        let cipher = self.cipher(&aad, &value.kdf_salt, &value.nonce)?;
        let payload = Payload {
            msg: &value.ciphertext,
            aad: &aad,
        };
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&value.nonce), payload)
            .map_err(|_| CredentialSealingFailure::Decryption)?;
        let value =
            String::from_utf8(plaintext).map_err(|_| CredentialSealingFailure::Decryption)?;
        Ok(SecretString::from(value))
    }

    fn cipher(
        &self,
        associated_data: &[u8],
        kdf_salt: &[u8; KDF_SALT_LEN_BYTES],
        nonce: &[u8; CREDENTIAL_NONCE_LEN_BYTES],
    ) -> Result<ChaCha20Poly1305, CredentialSealingFailure> {
        let key_material = self.derive_credential_key(associated_data, kdf_salt, nonce)?;
        ChaCha20Poly1305::new_from_slice(&key_material[..])
            .map_err(|_| CredentialSealingFailure::InvalidCipherKey)
    }

    fn derive_credential_key(
        &self,
        associated_data: &[u8],
        kdf_salt: &[u8; KDF_SALT_LEN_BYTES],
        nonce: &[u8; CREDENTIAL_NONCE_LEN_BYTES],
    ) -> Result<Zeroizing<[u8; CREDENTIAL_KEY_LEN_BYTES]>, CredentialSealingFailure> {
        let argon2 = argon2_for_key_len(CREDENTIAL_KEY_LEN_BYTES)?;
        let mut output = Zeroizing::new([0_u8; CREDENTIAL_KEY_LEN_BYTES]);
        let mut salt = Zeroizing::new(Vec::with_capacity(
            kdf_salt.len() + nonce.len() + associated_data.len(),
        ));
        salt.extend_from_slice(kdf_salt);
        salt.extend_from_slice(nonce);
        salt.extend_from_slice(associated_data);
        argon2
            .hash_password_into(&self.inner.root_material[..], &salt, &mut *output)
            .map_err(|err| CredentialSealingFailure::KeyDerivation {
                detail: err.to_string(),
            })?;
        Ok(output)
    }
}

/// Seal one credential value according to Tanren's v1 user-credential policy.
///
/// This helper is kept for compatibility paths; it initializes a reusable
/// sealer from env and delegates to [`CredentialValueSealer::seal`].
///
/// # Errors
///
/// Returns [`CredentialSealingFailure`] when passphrase configuration is
/// missing/invalid, key derivation fails, or encryption fails.
pub async fn seal_user_credential_value_from_env(
    context: UserCredentialSealContext,
    value: SecretString,
) -> Result<SealedUserCredentialValue, CredentialSealingFailure> {
    let sealer = CredentialValueSealer::from_env()?;
    sealer.seal(context, value).await
}

fn derive_root_material(
    passphrase: &[u8],
) -> Result<Zeroizing<[u8; CREDENTIAL_KEY_LEN_BYTES]>, CredentialSealingFailure> {
    let argon2 = argon2_for_key_len(CREDENTIAL_KEY_LEN_BYTES)?;
    let mut output = Zeroizing::new([0_u8; CREDENTIAL_KEY_LEN_BYTES]);
    argon2
        .hash_password_into(passphrase, ROOT_MATERIAL_SALT_V1, &mut *output)
        .map_err(|err| CredentialSealingFailure::KeyDerivation {
            detail: err.to_string(),
        })?;
    Ok(output)
}

fn argon2_for_key_len(key_len: usize) -> Result<Argon2<'static>, CredentialSealingFailure> {
    let params = Params::new(
        ARGON2_MEMORY_COST_KIB,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(key_len),
    )
    .map_err(|err| CredentialSealingFailure::InvalidKdfParameters {
        detail: err.to_string(),
    })?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn associated_data(
    context: &UserCredentialSealContext,
    credential_kind_wire: &str,
    seal_version: CredentialSealVersion,
) -> Zeroizing<Vec<u8>> {
    let (owner_scope_wire, owner_account_id) = owner_scope_parts(context.owner_scope);
    let mut aad = Zeroizing::new(Vec::with_capacity(192 + credential_kind_wire.len()));
    let version_wire = seal_version.as_i16().to_string();
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
    aad.extend_from_slice(version_wire.as_bytes());
    aad.push(0);
    aad.extend_from_slice(ROOT_MATERIAL_DOMAIN_V1);
    aad
}

fn owner_scope_parts(owner_scope: OwnerScope) -> (&'static str, AccountId) {
    match owner_scope {
        OwnerScope::User { account_id } => ("user", account_id),
    }
}
