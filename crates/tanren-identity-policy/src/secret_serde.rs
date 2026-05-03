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

use secrecy::{ExposeSecret, SecretString};
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

/// Re-serialize a [`SecretString`] by exposing its inner value as a
/// JSON string.
///
/// **Use sparingly.** This exists for outbound contracts that need to
/// propagate a credential (rare); most consumers should keep secrets
/// off the wire entirely.
///
/// # Errors
///
/// Returns the serializer's error type if the underlying writer fails.
pub fn serialize_password_expose<S>(value: &SecretString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(value.expose_secret())
}

/// Serialize a [`SessionToken`] for an outbound contract by exposing
/// its inner string. Inbound deserialization is provided by the
/// type's own `Deserialize` impl.
///
/// # Errors
///
/// Returns the serializer's error type if the underlying writer fails.
pub fn serialize_session_token<S>(value: &SessionToken, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(value.expose_secret())
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
    Ok(SessionToken::from_secret(SecretString::from(raw)))
}
