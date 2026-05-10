//! Serde adapters for secret-bearing fields.
//!
//! `secrecy::SecretString` deliberately omits a `Serialize` impl — the
//! marker `SerializableSecret` trait is opt-in to prevent accidental
//! exfiltration. The contract crate's `SignUpRequest` /
//! `SignInRequest` / `AcceptInvitationRequest` shapes need to receive
//! a plaintext password from the wire (Deserialize) without
//! re-emitting it (Serialize). These helpers are the explicit, audited
//! seam.
//!
//! See `profiles/rust-cargo/architecture/secrets-handling.md`.

use secrecy::SecretString;
use serde::{Deserializer, Serializer};

use crate::SessionToken;

/// Deserialize a JSON string field directly into [`SecretString`].
///
/// # Errors
///
/// Returns the deserializer's error type if the underlying value is not
/// a JSON string.
pub fn deserialize_password<'de, D>(deserializer: D) -> Result<SecretString, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
    Ok(SecretString::from(raw))
}

/// Serialize a password-shaped field as a fixed redaction marker.
///
/// This prevents accidental plaintext exfiltration through ordinary
/// `serde_json::to_*` calls over request DTOs.
///
/// # Errors
///
/// Returns the serializer's error type if the underlying writer fails.
pub fn serialize_password_redacted<S>(
    _value: &SecretString,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str("<redacted>")
}

/// Serialize a [`SessionToken`] for an outbound contract by exposing
/// its inner string. This must only be used by explicit bearer
/// transport wrappers.
///
/// # Errors
///
/// Returns the serializer's error type if the underlying writer fails.
pub fn serialize_session_token_expose<S>(
    value: &SessionToken,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(value.expose_secret())
}

/// Serialize a [`SessionToken`] as a fixed redaction marker.
///
/// # Errors
///
/// Returns the serializer's error type if the underlying writer fails.
pub fn serialize_session_token_redacted<S>(
    _value: &SessionToken,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str("<redacted>")
}

/// Deserialize a [`SessionToken`] from a JSON string.
///
/// # Errors
///
/// Returns the deserializer's error type if the underlying value is not
/// a JSON string.
pub fn deserialize_session_token<'de, D>(deserializer: D) -> Result<SessionToken, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
    SessionToken::parse(&raw).map_err(serde::de::Error::custom)
}
