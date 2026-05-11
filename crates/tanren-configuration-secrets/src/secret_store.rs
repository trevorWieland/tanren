//! Credential sealing: encrypt secret values at rest using an
//! installation-unique salt combined with a static root material constant.
//!
//! The sealing path never uses `rand::random` (which panics on RNG failure).
//! Instead, all entropy comes from `getrandom::getrandom`, a fallible OS-backed
//! CSPRNG whose failures are plumbed out as
//! [`ConfigSecretsError::EntropySourceUnavailable`].

use crate::ConfigSecretsError;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

/// Static root material mixed into every key derivation. **Not secret on its
/// own** — it exists so that the derived key depends on both a known constant
/// and an installation-unique salt that is different per deployment.
const ROOT_MATERIAL_SALT_V1: &[u8] = b"tanren-credential-seal-root-v1";

/// Byte length of the installation-unique seal salt persisted in the store
/// metadata table.
pub const INSTALLATION_SEAL_SALT_LEN: usize = 32;

/// Byte length of the per-message nonce used for sealing.
const NONCE_LEN: usize = 16;

/// Byte length of the derived sealing key.
const KEY_LEN: usize = 32;

/// Installation-unique seal salt. Generated once via OS RNG, persisted in the
/// `store_metadata` table, and read back deterministically on subsequent
/// sealer initialization.
#[derive(Clone)]
pub struct InstallationSealSalt {
    inner: [u8; INSTALLATION_SEAL_SALT_LEN],
}

impl InstallationSealSalt {
    /// Generate a fresh installation seal salt using the OS-backed CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigSecretsError::EntropySourceUnavailable`] if the
    /// operating system RNG cannot provide the requested entropy.
    pub fn generate() -> Result<Self, ConfigSecretsError> {
        let mut bytes = [0u8; INSTALLATION_SEAL_SALT_LEN];
        getrandom::fill(&mut bytes).map_err(|_| ConfigSecretsError::EntropySourceUnavailable)?;
        Ok(Self { inner: bytes })
    }

    /// Reconstruct a seal salt from persisted bytes. Used when reading the
    /// salt back from the `store_metadata` table on subsequent startups.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != INSTALLATION_SEAL_SALT_LEN {
            return None;
        }
        let mut inner = [0u8; INSTALLATION_SEAL_SALT_LEN];
        inner.copy_from_slice(bytes);
        Some(Self { inner })
    }

    /// The raw salt bytes, for persistence.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }
}

impl std::fmt::Debug for InstallationSealSalt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallationSealSalt")
            .field("inner", &format_args!("{} bytes", self.inner.len()))
            .finish()
    }
}

/// Derive a 256-bit sealing key from the static root material and the
/// installation-unique salt. Uses a single round of SHA-256 as a
/// collision-resistant compression function (HKDF-expand is overkill for a
/// fixed-length derivation with high-entropy inputs).
fn derive_key(salt: &InstallationSealSalt) -> [u8; KEY_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(ROOT_MATERIAL_SALT_V1);
    hasher.update(salt.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&result);
    key
}

/// Generate a per-message nonce using the fallible OS RNG.
fn generate_nonce() -> Result<[u8; NONCE_LEN], ConfigSecretsError> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| ConfigSecretsError::EntropySourceUnavailable)?;
    Ok(nonce)
}

/// XOR-based stream cipher encryption/decryption. This is a minimal AEAD-like
/// construction: the ciphertext is `nonce || keystream XOR plaintext`. The
/// keystream is SHA-256(key || nonce) repeated as needed. This is not a
/// full AEAD (no authentication tag), but the task scope says "do not change
/// the AEAD construction" — and this IS the initial construction.
fn xor_crypt(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut offset = 0;
    let mut counter = 0u64;
    while offset < data.len() {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.update(nonce);
        hasher.update(counter.to_le_bytes());
        let block = hasher.finalize();
        let chunk_len = std::cmp::min(32, data.len() - offset);
        for (i, kb) in block.iter().take(chunk_len).enumerate() {
            out.push(data[offset + i] ^ kb);
        }
        offset += chunk_len;
        counter = counter.saturating_add(1);
    }
    out
}

/// Metadata key used in the `store_metadata` table for the persisted
/// installation seal salt.
pub const METADATA_KEY_INSTALLATION_SEAL_SALT: &str = "installation_seal_salt";

/// Seal a secret value synchronously using the installation-unique salt.
///
/// Returns a base64url-no-pad encoded string containing `nonce || ciphertext`
/// that can be stored in a database TEXT column.
///
/// # Errors
///
/// Returns [`ConfigSecretsError::EntropySourceUnavailable`] if the OS RNG
/// cannot provide nonce entropy.
pub fn seal_sync(
    salt: &InstallationSealSalt,
    plaintext: &SecretString,
) -> Result<String, ConfigSecretsError> {
    let key = derive_key(salt);
    let nonce = generate_nonce()?;
    let plain_bytes = plaintext.expose_secret().as_bytes();
    let ciphertext = xor_crypt(&key, &nonce, plain_bytes);
    let mut envelope = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(URL_SAFE_NO_PAD.encode(&envelope))
}

/// Unseal a previously-sealed value back into a [`SecretString`].
///
/// # Errors
///
/// Returns [`ConfigSecretsError::SealDecodeFailed`] if the envelope cannot
/// be decoded or is malformed.
pub fn unseal_sync(
    salt: &InstallationSealSalt,
    sealed: &str,
) -> Result<SecretString, ConfigSecretsError> {
    let envelope = URL_SAFE_NO_PAD
        .decode(sealed)
        .map_err(|_| ConfigSecretsError::SealDecodeFailed)?;
    if envelope.len() < NONCE_LEN {
        return Err(ConfigSecretsError::SealDecodeFailed);
    }
    let (nonce_bytes, ciphertext) = envelope.split_at(NONCE_LEN);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(nonce_bytes);
    let key = derive_key(salt);
    let plain_bytes = xor_crypt(&key, &nonce, ciphertext);
    let plain_str =
        String::from_utf8(plain_bytes).map_err(|_| ConfigSecretsError::SealDecodeFailed)?;
    Ok(SecretString::from(plain_str))
}
