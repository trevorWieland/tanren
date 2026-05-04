//! Argon2id password verifier — the canonical [`CredentialVerifier`] impl.
//!
//! Production parameters track the OWASP 2025 floor (m = 19 MiB, t = 2,
//! p = 1). The [`Argon2idVerifier::fast_for_tests`] preset uses cheap
//! parameters so BDD scenarios stay fast; it is gated behind the
//! `test-hooks` Cargo feature so production binaries cannot accidentally
//! reach for it.
//!
//! Hashes are written and read in the [PHC string format] (e.g.
//! `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`); salt is embedded in
//! the string so the persistence layer carries a single TEXT column,
//! not a hash + salt pair.
//!
//! [PHC string format]:
//!     https://github.com/P-H-C/phc-string-format/blob/master/phc-sf-spec.md

use argon2::{Algorithm, Argon2, Params, Version};
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use secrecy::{ExposeSecret, SecretString};

use crate::{CredentialVerifier, IdentityError};

/// OWASP 2025 password-hashing floor expressed as Argon2id parameters.
const OWASP_2025_M_COST_KIB: u32 = 19_456;
const OWASP_2025_T_COST: u32 = 2;
const OWASP_2025_P_COST: u32 = 1;

/// Argon2id-backed [`CredentialVerifier`].
///
/// Cheap to clone — internally just a snapshot of the configured
/// parameters. Each `hash` / `verify` call constructs a fresh
/// [`Argon2`] instance from those parameters.
#[derive(Debug, Clone)]
pub struct Argon2idVerifier {
    params: Params,
}

impl Argon2idVerifier {
    /// Production verifier matching the OWASP 2025 floor: `m = 19 MiB`,
    /// `t = 2`, `p = 1`. See
    /// `docs/architecture/subsystems/identity-policy.md` § "Canonical
    /// credential and session decisions".
    #[must_use]
    pub fn production() -> Self {
        let params = Params::new(
            OWASP_2025_M_COST_KIB,
            OWASP_2025_T_COST,
            OWASP_2025_P_COST,
            None,
        )
        .expect("OWASP 2025 Argon2id parameters are statically valid");
        Self { params }
    }

    /// Cheap parameters for BDD scenarios. Gated on the `test-hooks`
    /// feature so production binaries cannot accidentally hash with
    /// these.
    #[cfg(any(test, feature = "test-hooks"))]
    #[must_use]
    pub fn fast_for_tests() -> Self {
        let params = Params::new(8, 1, 1, None)
            .expect("fast_for_tests Argon2id parameters are statically valid");
        Self { params }
    }

    fn argon2(&self) -> Argon2<'_> {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, self.params.clone())
    }
}

impl CredentialVerifier for Argon2idVerifier {
    fn hash(&self, password: &SecretString) -> Result<String, IdentityError> {
        let salt = SaltString::generate(&mut OsRng);
        let phc = self
            .argon2()
            .hash_password(password.expose_secret().as_bytes(), &salt)
            .map_err(|err| IdentityError::HashFailed(err.to_string()))?;
        Ok(phc.to_string())
    }

    fn verify(&self, password: &SecretString, stored: &str) -> Result<(), IdentityError> {
        let parsed =
            PasswordHash::new(stored).map_err(|err| IdentityError::HashFailed(err.to_string()))?;
        self.argon2()
            .verify_password(password.expose_secret().as_bytes(), &parsed)
            .map_err(|_| IdentityError::InvalidCredential)
    }
}
