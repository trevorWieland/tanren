//! `SessionToken` — opaque CSPRNG-minted credential for sign-up /
//! sign-in / accept-invitation responses.
//!
//! Split out of `lib.rs` so the identity-policy crate stays under the
//! workspace 500-line line-budget. The contract is unchanged: `Debug`
//! redacts, `Display` is intentionally absent, and the only access
//! point is [`SessionToken::expose_secret`].

use crate::ValidationError;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use schemars::JsonSchema;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use utoipa::ToSchema;

const SESSION_TOKEN_BYTES_LEN: usize = 32;

/// Opaque session token — 256 bits of CSPRNG randomness encoded
/// base64url-no-pad and wrapped in [`SecretString`] so accidental
/// `Display` / `Debug` / `Serialize` calls cannot leak the credential.
///
/// Construction is one of:
///
/// - [`SessionToken::generate`] — fresh CSPRNG token at sign-up / sign-in /
///   accept-invitation time.
/// - [`SessionToken::from_secret`] — wrap an already-realised secret (used
///   by the store layer when re-hydrating a session row).
///
/// Access to the inner string is only via [`SessionToken::expose_secret`].
/// `Debug` prints `SessionToken(<redacted>)`; `Display` is intentionally
/// not implemented.
#[derive(Clone)]
pub struct SessionToken(SecretString);

impl SessionToken {
    /// Mint a fresh session token: 32 random bytes, URL-safe base64
    /// (no padding).
    #[must_use]
    pub fn generate() -> Self {
        let bytes: [u8; 32] = rand::random();
        let encoded = URL_SAFE_NO_PAD.encode(bytes);
        Self(SecretString::from(encoded))
    }

    /// Wrap an existing secret. Used by the store layer when re-hydrating
    /// a session row from the database; production handlers should call
    /// [`SessionToken::generate`] instead.
    #[must_use]
    pub const fn from_secret(secret: SecretString) -> Self {
        Self(secret)
    }

    /// Parse and validate a bearer token string.
    ///
    /// Accepted shape is exactly 32 random bytes encoded as base64url-no-pad.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::SessionTokenInvalid`] when the value is empty
    /// or is not canonical base64url-no-pad with a 32-byte decoded payload.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::SessionTokenInvalid);
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(trimmed.as_bytes())
            .map_err(|_| ValidationError::SessionTokenInvalid)?;
        if decoded.len() != SESSION_TOKEN_BYTES_LEN {
            return Err(ValidationError::SessionTokenInvalid);
        }
        if URL_SAFE_NO_PAD.encode(decoded.as_slice()) != trimmed {
            return Err(ValidationError::SessionTokenInvalid);
        }
        Ok(Self(SecretString::from(trimmed.to_owned())))
    }

    /// Expose the inner token string. The only access point — every
    /// other surface (Debug, Display, Serialize) intentionally redacts.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionToken(<redacted>)")
    }
}

impl<'de> Deserialize<'de> for SessionToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for SessionToken {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SessionToken")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

impl utoipa::PartialSchema for SessionToken {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        // The token serialises as a plain string; the inner secret is
        // never exposed. utoipa needs a string-shaped schema to reflect
        // that on the wire — `Debug` redacts in-process but the wire
        // form is a string for bearer-flow callers.
        String::schema()
    }
}

impl ToSchema for SessionToken {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SessionToken")
    }
}
