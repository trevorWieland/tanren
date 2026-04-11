//! Validated wrapper types enforcing construction-time invariants.
//!
//! Every value of these types satisfies its invariant at the type level.
//! Callers cannot construct invalid values because the only route to
//! construction returns `Result<Self, DomainError>` and deserialization
//! re-runs the same validation.

use serde::{Deserialize, Serialize};

use crate::errors::DomainError;

/// A string guaranteed to be non-empty (and not whitespace-only).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_rejects_empty() {
        assert!(NonEmptyString::try_new("").is_err());
        assert!(NonEmptyString::try_new("   ").is_err());
        assert!(NonEmptyString::try_new("\n\t").is_err());
    }

    #[test]
    fn non_empty_accepts_value() {
        let s = NonEmptyString::try_new("hello").expect("valid");
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn non_empty_serde_roundtrip() {
        let s = NonEmptyString::try_new("hello").expect("valid");
        let json = serde_json::to_string(&s).expect("serialize");
        assert_eq!(json, "\"hello\"");
        let back: NonEmptyString = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(s, back);
    }

    #[test]
    fn non_empty_deserialize_rejects_empty() {
        let err = serde_json::from_str::<NonEmptyString>("\"\"");
        assert!(err.is_err());
    }

    #[test]
    fn timeout_rejects_zero() {
        assert!(TimeoutSecs::try_new(0).is_err());
    }

    #[test]
    fn timeout_accepts_positive() {
        let t = TimeoutSecs::try_new(30).expect("valid");
        assert_eq!(t.get(), 30);
    }

    #[test]
    fn timeout_serde_roundtrip() {
        let t = TimeoutSecs::try_new(60).expect("valid");
        let json = serde_json::to_string(&t).expect("serialize");
        assert_eq!(json, "60");
        let back: TimeoutSecs = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(t, back);
    }

    #[test]
    fn timeout_deserialize_rejects_zero() {
        let err = serde_json::from_str::<TimeoutSecs>("0");
        assert!(err.is_err());
    }
}
