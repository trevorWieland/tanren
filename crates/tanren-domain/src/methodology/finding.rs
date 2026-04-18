//! Findings — typed results of audit, adherence, demo, and investigation.
//!
//! A methodology [`Finding`] is richer than the legacy gate-level
//! `crate::payloads::Finding` (which remains for step-level worker
//! payloads). It carries provenance (which phase produced it), linkage
//! (which task it attaches to, if any), rubric pillar (for audit
//! findings), and standard reference (for adherence findings).
//!
//! Severity invariants:
//! - `FixNow` blocks the relevant completion guard (`Audited` for audit
//!   findings, `Adherent` for adherence findings) until resolved.
//! - `Defer` emits a backlog issue; disallowed on `critical` adherence
//!   standards (enforced at tool call in `app-services`).
//! - `Note` / `Question` are informational; they never block a guard.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{FindingId, SpecId, TaskId};
use crate::validated::NonEmptyString;

/// Severity of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    /// Must be addressed before the guard can be satisfied.
    FixNow,
    /// Acceptable to defer; emits a backlog issue.
    Defer,
    /// Informational; does not block any guard.
    Note,
    /// Open question for the reviewer; does not block any guard.
    Question,
}

impl FindingSeverity {
    /// Short stable `snake_case` tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::FixNow => "fix_now",
            Self::Defer => "defer",
            Self::Note => "note",
            Self::Question => "question",
        }
    }

    /// True for severities that block a completion guard.
    #[must_use]
    pub const fn is_blocking(self) -> bool {
        matches!(self, Self::FixNow)
    }
}

impl std::fmt::Display for FindingSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
    }
}

/// Severity allowed on adherence findings only.
///
/// Adherence is deterministic standards-compliance, so the allowed set
/// is intentionally narrower than generic audit/demo findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdherenceSeverity {
    /// Violation blocks adherence guard satisfaction.
    FixNow,
    /// Real violation explicitly deferred.
    Defer,
}

impl AdherenceSeverity {
    /// Stable `snake_case` tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::FixNow => "fix_now",
            Self::Defer => "defer",
        }
    }

    /// Convert to the broader finding-severity type.
    #[must_use]
    pub const fn as_finding_severity(self) -> FindingSeverity {
        match self {
            Self::FixNow => FindingSeverity::FixNow,
            Self::Defer => FindingSeverity::Defer,
        }
    }
}

impl std::fmt::Display for AdherenceSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
    }
}

/// Where a finding came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FindingSource {
    /// Emitted from `audit-task` or `audit-spec`.
    Audit {
        phase: NonEmptyString,
        /// Rubric pillar this finding supports (score < target).
        pillar: Option<NonEmptyString>,
    },
    /// Emitted from `adhere-task` or `adhere-spec`.
    Adherence { standard: StandardRef },
    /// Emitted from `run-demo` for a failed demo step.
    Demo {
        run_id: NonEmptyString,
        step_id: NonEmptyString,
    },
    /// Emitted from `investigate` during diagnosis.
    Investigation { loop_index: u16 },
    /// Emitted from `triage-audits` when re-classifying an existing finding.
    Triage,
    /// Emitted from `sync-roadmap` or `handle-feedback`.
    Feedback { source_ref: NonEmptyString },
}

/// Reference to the standard a finding is rooted in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StandardRef {
    pub name: NonEmptyString,
    pub category: NonEmptyString,
}

/// Canonical finding record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    pub id: FindingId,
    pub spec_id: SpecId,
    pub severity: FindingSeverity,
    pub title: NonEmptyString,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_numbers: Vec<u32>,
    pub source: FindingSource,
    pub attached_task: Option<TaskId>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_tag_and_blocking() {
        assert_eq!(FindingSeverity::FixNow.tag(), "fix_now");
        assert_eq!(FindingSeverity::Defer.tag(), "defer");
        assert_eq!(FindingSeverity::Note.tag(), "note");
        assert_eq!(FindingSeverity::Question.tag(), "question");
        assert!(FindingSeverity::FixNow.is_blocking());
        assert!(!FindingSeverity::Defer.is_blocking());
        assert!(!FindingSeverity::Note.is_blocking());
        assert!(!FindingSeverity::Question.is_blocking());
    }

    #[test]
    fn severity_serde_roundtrip() {
        for s in [
            FindingSeverity::FixNow,
            FindingSeverity::Defer,
            FindingSeverity::Note,
            FindingSeverity::Question,
        ] {
            let json = serde_json::to_string(&s).expect("serialize");
            let back: FindingSeverity = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(s, back);
        }
    }

    #[test]
    fn adherence_severity_roundtrip() {
        for s in [AdherenceSeverity::FixNow, AdherenceSeverity::Defer] {
            let json = serde_json::to_string(&s).expect("serialize");
            let back: AdherenceSeverity = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(s, back);
        }
    }

    #[test]
    fn finding_source_tagged_representation() {
        let src = FindingSource::Audit {
            phase: NonEmptyString::try_new("audit-task").expect("phase"),
            pillar: Some(NonEmptyString::try_new("security").expect("pillar")),
        };
        let json = serde_json::to_value(&src).expect("serialize");
        assert_eq!(json["kind"], "audit");
        assert_eq!(json["phase"], "audit-task");
        assert_eq!(json["pillar"], "security");
    }

    #[test]
    fn standard_ref_roundtrip() {
        let sr = StandardRef {
            name: NonEmptyString::try_new("tokio-runtime").expect("name"),
            category: NonEmptyString::try_new("async").expect("cat"),
        };
        let json = serde_json::to_string(&sr).expect("serialize");
        let back: StandardRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sr, back);
    }
}
