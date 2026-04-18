//! Standards — adherence-layer configuration.
//!
//! A [`Standard`] is a named, categorized, importance-tagged rule with
//! glob-, language-, and domain-based applicability filters. Adherence
//! phases (`adhere-task`, `adhere-spec`) compute the relevant-standard
//! set per `docs/architecture/adherence.md` §4.1 and then check each
//! against the spec's touched files.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::validated::NonEmptyString;

/// Importance level of a standard. `Critical` disallows `defer` severity.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StandardImportance {
    Low,
    Medium,
    High,
    /// Cannot be deferred — findings against critical standards must be
    /// `fix_now`. Enforced at tool call in `app-services`.
    Critical,
}

impl StandardImportance {
    /// True iff findings against a standard at this importance cannot be
    /// deferred to the backlog.
    #[must_use]
    pub const fn disallows_defer(self) -> bool {
        matches!(self, Self::Critical)
    }
}

/// Canonical standard record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Standard {
    pub name: NonEmptyString,
    pub category: NonEmptyString,
    /// Globs matched against the spec's touched files (e.g. `"**/*.rs"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applies_to: Vec<String>,
    /// Languages the standard applies to (e.g. `["rust"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applies_to_languages: Vec<String>,
    /// Domain tags (e.g. `["async", "storage"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applies_to_domains: Vec<String>,
    pub importance: StandardImportance,
    /// Free-text body rendered into `tanren/standards/<cat>/<name>.md`.
    pub body: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_disallows_defer() {
        assert!(StandardImportance::Critical.disallows_defer());
        assert!(!StandardImportance::High.disallows_defer());
        assert!(!StandardImportance::Medium.disallows_defer());
        assert!(!StandardImportance::Low.disallows_defer());
    }

    #[test]
    fn importance_ordering() {
        assert!(StandardImportance::Low < StandardImportance::Medium);
        assert!(StandardImportance::Medium < StandardImportance::High);
        assert!(StandardImportance::High < StandardImportance::Critical);
    }

    #[test]
    fn standard_roundtrip() {
        let s = Standard {
            name: NonEmptyString::try_new("tokio-runtime").expect("name"),
            category: NonEmptyString::try_new("async").expect("cat"),
            applies_to: vec!["**/*.rs".into()],
            applies_to_languages: vec!["rust".into()],
            applies_to_domains: vec!["async".into()],
            importance: StandardImportance::High,
            body: "Prefer multi-thread tokio runtime for server workloads.".into(),
        };
        let json = serde_json::to_string(&s).expect("serialize");
        let back: Standard = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(s, back);
    }
}
