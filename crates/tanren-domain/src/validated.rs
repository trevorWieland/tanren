//! Validated wrapper types enforcing construction-time invariants.
//!
//! Every value of these types satisfies its invariant at the type level.
//! Callers cannot construct invalid values because the only route to
//! construction returns `Result<Self, DomainError>` and deserialization
//! re-runs the same validation — including the `serde_json::Value`
//! path used by `SeaORM`'s `JsonBinary` columns.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::DomainError;

/// A string guaranteed to be non-empty (and not whitespace-only).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    /// Attempt to construct a `NonEmptyString`.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidValue`] if the input is empty or
    /// contains only whitespace.
    pub fn try_new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::InvalidValue {
                field: "non_empty_string".into(),
                reason: "must not be empty or whitespace".into(),
            });
        }
        Ok(Self(value))
    }

    /// Borrow the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the inner `String`.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for NonEmptyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for NonEmptyString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::try_new(raw).map_err(serde::de::Error::custom)
    }
}

/// A timeout expressed in seconds, guaranteed to be strictly positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct TimeoutSecs(u64);

impl TimeoutSecs {
    /// Attempt to construct a `TimeoutSecs`.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidValue`] if `secs` is zero.
    pub fn try_new(secs: u64) -> Result<Self, DomainError> {
        if secs == 0 {
            return Err(DomainError::InvalidValue {
                field: "timeout_secs".into(),
                reason: "must be strictly positive".into(),
            });
        }
        Ok(Self(secs))
    }

    /// Return the value as seconds.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for TimeoutSecs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s", self.0)
    }
}

impl<'de> Deserialize<'de> for TimeoutSecs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = u64::deserialize(deserializer)?;
        Self::try_new(raw).map_err(serde::de::Error::custom)
    }
}

/// A floating-point value guaranteed to be finite (not `NaN` or infinity).
///
/// Persisted event and result payload fields use this wrapper instead of
/// raw `f64` so non-finite values can never be written to the event log.
/// `serde_json` silently encodes `NaN` / `Infinity` as `null`, which then
/// fails to deserialize — a silent-write / hard-fail-on-read pattern
/// that would break the `SeaORM` JSON round-trip contract. The custom
/// `Deserialize` impl reuses `try_new` so both `from_str` and `from_value`
/// enforce the invariant and surface the domain-level error message
/// rather than the opaque serde default.
///
/// `FiniteF64` intentionally does **not** impl `Eq`, `Ord`, or `Hash`
/// because `f64` does not satisfy those contracts. It does impl
/// `PartialEq` and `PartialOrd`, which is sufficient for every current
/// consumer (no `DomainEvent` / payload variant derives `Eq`).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FiniteF64(f64);

impl FiniteF64 {
    /// Attempt to construct a `FiniteF64`.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidValue`] if `value` is `NaN`,
    /// positive infinity, or negative infinity.
    pub fn try_new(value: f64) -> Result<Self, DomainError> {
        if !value.is_finite() {
            return Err(DomainError::InvalidValue {
                field: "finite_f64".into(),
                reason: "must be a finite number (not NaN or infinity)".into(),
            });
        }
        Ok(Self(value))
    }

    /// Return the inner `f64` value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for FiniteF64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<FiniteF64> for f64 {
    fn from(value: FiniteF64) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for FiniteF64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = f64::deserialize(deserializer)?;
        Self::try_new(raw).map_err(serde::de::Error::custom)
    }
}
