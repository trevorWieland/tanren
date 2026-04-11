//! Validated wrapper types enforcing construction-time invariants.
//!
//! Every value of these types satisfies its invariant at the type level.
//! Callers cannot construct invalid values because the only route to
//! construction returns `Result<Self, DomainError>` and deserialization
//! re-runs the same validation — including the `serde_json::Value`
//! path used by `SeaORM`'s `JsonBinary` columns.

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

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    // -- NonEmptyString ---------------------------------------------------

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
    fn non_empty_from_value_roundtrip() {
        // `SeaORM` JsonBinary uses to_value / from_value — certify it here.
        let s = NonEmptyString::try_new("hello").expect("valid");
        let value = serde_json::to_value(&s).expect("to_value");
        assert_eq!(value, Value::String("hello".into()));
        let back: NonEmptyString = serde_json::from_value(value).expect("from_value");
        assert_eq!(s, back);
    }

    #[test]
    fn non_empty_from_value_rejects_empty() {
        let err = serde_json::from_value::<NonEmptyString>(Value::String(String::new()))
            .expect_err("empty string must be rejected via from_value");
        let msg = err.to_string();
        assert!(
            msg.contains("non_empty_string") || msg.contains("must not be empty"),
            "expected DomainError::InvalidValue content in {msg}"
        );
    }

    #[test]
    fn non_empty_from_value_rejects_whitespace() {
        let err = serde_json::from_value::<NonEmptyString>(Value::String("   \n".into()))
            .expect_err("whitespace string must be rejected via from_value");
        let msg = err.to_string();
        assert!(
            msg.contains("non_empty_string") || msg.contains("must not be empty"),
            "expected DomainError::InvalidValue content in {msg}"
        );
    }

    // -- TimeoutSecs ------------------------------------------------------

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

    #[test]
    fn timeout_from_value_roundtrip() {
        let t = TimeoutSecs::try_new(3600).expect("valid");
        let value = serde_json::to_value(t).expect("to_value");
        let back: TimeoutSecs = serde_json::from_value(value).expect("from_value");
        assert_eq!(t, back);
    }

    #[test]
    fn timeout_from_value_rejects_zero() {
        let err = serde_json::from_value::<TimeoutSecs>(Value::from(0_u64))
            .expect_err("zero must be rejected via from_value");
        let msg = err.to_string();
        assert!(
            msg.contains("timeout_secs") || msg.contains("strictly positive"),
            "expected DomainError::InvalidValue content in {msg}"
        );
    }

    // -- FiniteF64 --------------------------------------------------------

    /// Compare two `f64` values by bit pattern. `FiniteF64` excludes
    /// `NaN`, so bitwise equality is an exact value comparison without
    /// tripping clippy's `float_cmp` lint.
    fn bits_eq(a: f64, b: f64) -> bool {
        a.to_bits() == b.to_bits()
    }

    #[test]
    fn finite_accepts_zero() {
        let v = FiniteF64::try_new(0.0).expect("zero is finite");
        assert!(bits_eq(v.get(), 0.0));
    }

    #[test]
    fn finite_accepts_negative() {
        // Negative values are legal at the type level. Stricter
        // wrappers (e.g. non-negative durations) are a future concern.
        let v = FiniteF64::try_new(-1.5).expect("negative is finite");
        assert!(bits_eq(v.get(), -1.5));
    }

    #[test]
    fn finite_accepts_large_positive() {
        let v = FiniteF64::try_new(1e308).expect("very large but finite");
        assert!(bits_eq(v.get(), 1e308));
    }

    #[test]
    fn finite_rejects_nan() {
        let err = FiniteF64::try_new(f64::NAN).expect_err("NaN must be rejected");
        assert!(matches!(&err, DomainError::InvalidValue { field, reason }
                if field == "finite_f64" && reason.contains("finite")));
    }

    #[test]
    fn finite_rejects_positive_infinity() {
        assert!(FiniteF64::try_new(f64::INFINITY).is_err());
    }

    #[test]
    fn finite_rejects_negative_infinity() {
        assert!(FiniteF64::try_new(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn finite_from_str_roundtrip() {
        let v = FiniteF64::try_new(45.2).expect("valid");
        let json = serde_json::to_string(&v).expect("serialize");
        assert_eq!(json, "45.2");
        let back: FiniteF64 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn finite_from_str_rejects_null() {
        // serde_json encodes NaN/Infinity as null on write — this must
        // not round-trip back into a FiniteF64 silently.
        assert!(serde_json::from_str::<FiniteF64>("null").is_err());
    }

    #[test]
    fn finite_from_value_roundtrip() {
        // Direct `SeaORM` `JsonBinary` certification.
        let v = FiniteF64::try_new(12.5).expect("valid");
        let value = serde_json::to_value(v).expect("to_value");
        let back: FiniteF64 = serde_json::from_value(value).expect("from_value");
        assert_eq!(v, back);
    }

    #[test]
    fn finite_from_value_rejects_null() {
        let err = serde_json::from_value::<FiniteF64>(Value::Null)
            .expect_err("null must be rejected via from_value");
        let _ = err; // just need the error to exist
    }

    #[test]
    fn finite_display_forwards_to_f64() {
        // Use 2.5 (exact in binary floating point, not an approximation
        // of any well-known mathematical constant).
        assert_eq!(FiniteF64::try_new(2.5).expect("valid").to_string(), "2.5");
    }

    #[test]
    fn finite_into_f64() {
        let v = FiniteF64::try_new(7.0).expect("valid");
        let raw: f64 = v.into();
        assert!(bits_eq(raw, 7.0));
    }
}
