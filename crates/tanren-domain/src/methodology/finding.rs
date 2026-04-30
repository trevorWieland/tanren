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

use crate::ids::{CheckRunId, FindingId, SpecId, TaskId};
use crate::methodology::check::CheckKind;
use crate::methodology::phase_id::PhaseId;
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
        phase: PhaseId,
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
    /// Emitted when re-classifying an existing finding during project intake.
    Triage,
    /// Emitted from `handle-feedback` or external feedback reconciliation.
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

/// Derived lifecycle status of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    Open,
    Resolved,
    Reopened,
    Deferred,
    Superseded,
}

impl FindingStatus {
    /// True when this status blocks readiness for blocking severities.
    #[must_use]
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Open | Self::Reopened)
    }
}

/// Evidence attached to a lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindingLifecycleEvidence {
    pub summary: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_run_id: Option<CheckRunId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_kind: Option<CheckKind>,
}

/// A finding plus its current projected lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindingView {
    pub finding: Finding,
    pub status: FindingStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_evidence: Option<FindingLifecycleEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub superseded_by: Vec<FindingId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

impl FindingView {
    /// True iff this finding currently blocks readiness.
    #[must_use]
    pub fn is_open_blocking(&self) -> bool {
        self.status.is_open() && self.finding.severity.is_blocking()
    }
}
