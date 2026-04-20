//! Backlog issues.
//!
//! Issues are created by `triage-audits` (out-of-scope items from a
//! spec audit) and by `handle-feedback` (PR-review comments that should
//! not land in the current spec). They carry a provider-tagged reference
//! so the same model can address GitHub today and Linear in a follow-up
//! lane without a breaking schema change.
//!
//! Typed URL parsing lives in `app-services` (where a `url` crate
//! dependency is added at the boundary). The domain model keeps
//! [`NonEmptyString`] here so this crate stays workspace-dep-free.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{IssueId, SpecId};
use crate::validated::NonEmptyString;

/// Issue tracker backend. `#[non_exhaustive]` so adding Linear (or any
/// other provider) is a purely additive enum change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub enum IssueProvider {
    #[serde(rename = "github")]
    GitHub,
}

impl std::fmt::Display for IssueProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => f.write_str("github"),
        }
    }
}

/// Priority tag for a backlog issue.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IssuePriority {
    Low,
    Medium,
    High,
    Urgent,
}

/// Reference to an issue in an external tracker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueRef {
    pub provider: IssueProvider,
    /// Provider-native issue number (e.g. GitHub issue number).
    pub number: u32,
    /// Canonical URL. Always present post-creation. Validated by
    /// `app-services` at construction time; stored here as a
    /// [`NonEmptyString`] so `tanren-domain` remains dep-free.
    pub url: NonEmptyString,
}

/// Canonical issue record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    pub id: IssueId,
    pub origin_spec_id: SpecId,
    pub title: NonEmptyString,
    pub description: String,
    pub suggested_spec_scope: NonEmptyString,
    pub priority: IssuePriority,
    pub reference: IssueRef,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_serializes_as_github() {
        let json = serde_json::to_string(&IssueProvider::GitHub).expect("serialize");
        assert_eq!(json, "\"github\"");
    }

    #[test]
    fn priority_ordering() {
        assert!(IssuePriority::Low < IssuePriority::Medium);
        assert!(IssuePriority::Medium < IssuePriority::High);
        assert!(IssuePriority::High < IssuePriority::Urgent);
    }

    #[test]
    fn issue_ref_roundtrip() {
        let r = IssueRef {
            provider: IssueProvider::GitHub,
            number: 42,
            url: NonEmptyString::try_new("https://github.com/owner/repo/issues/42")
                .expect("non-empty"),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let back: IssueRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
    }
}
