//! Tool capabilities and per-phase capability scopes.
//!
//! A [`ToolCapability`] is the unit of authorization on the agent tool
//! surface. Each tool method in `app-services::methodology::service`
//! requires a specific capability; the MCP transport consults
//! `TANREN_PHASE_CAPABILITIES` (supplied by the orchestrator at dispatch)
//! to decide which tools are callable in the current phase.
//!
//! Per-phase defaults mirror
//! `docs/architecture/agent-tool-surface.md` §4 verbatim.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single authorization scope on the agent tool surface.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ToolCapability {
    // Core task operations (§3.1)
    TaskCreate,
    TaskStart,
    TaskComplete,
    TaskRevise,
    TaskAbandon,
    TaskRead,

    // Findings & rubric (§3.2)
    FindingAdd,
    RubricRecord,
    ComplianceRecord,

    // Spec frontmatter (§3.3)
    SpecFrontmatter,

    // Demo frontmatter (§3.4)
    DemoFrontmatter,
    DemoResults,

    // Signposts (§3.5)
    SignpostAdd,
    SignpostUpdate,

    // Phase lifecycle (§3.6)
    PhaseOutcome,
    PhaseEscalate,

    // Backlog (§3.7)
    IssueCreate,

    // Standards & adherence (§3.8)
    StandardRead,
    AdherenceRecord,

    // Handle-feedback (§3.6)
    FeedbackReply,
}

impl ToolCapability {
    /// Short stable `snake_case` tag. Matches the serde representation.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::TaskCreate => "task.create",
            Self::TaskStart => "task.start",
            Self::TaskComplete => "task.complete",
            Self::TaskRevise => "task.revise",
            Self::TaskAbandon => "task.abandon",
            Self::TaskRead => "task.read",
            Self::FindingAdd => "finding.add",
            Self::RubricRecord => "rubric.record",
            Self::ComplianceRecord => "compliance.record",
            Self::SpecFrontmatter => "spec.frontmatter",
            Self::DemoFrontmatter => "demo.frontmatter",
            Self::DemoResults => "demo.results",
            Self::SignpostAdd => "signpost.add",
            Self::SignpostUpdate => "signpost.update",
            Self::PhaseOutcome => "phase.outcome",
            Self::PhaseEscalate => "phase.escalate",
            Self::IssueCreate => "issue.create",
            Self::StandardRead => "standard.read",
            Self::AdherenceRecord => "adherence.record",
            Self::FeedbackReply => "feedback.reply",
        }
    }
}

impl std::fmt::Display for ToolCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
    }
}

/// Ordered set of capabilities granted for one phase.
///
/// The set is always materialized as a `BTreeSet<ToolCapability>` so the
/// representation is deterministic for hashing and snapshot tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct CapabilityScope(pub BTreeSet<ToolCapability>);

impl CapabilityScope {
    /// Construct from any iterator of capabilities.
    pub fn from_iter_caps<I: IntoIterator<Item = ToolCapability>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }

    /// True iff the scope grants `cap`.
    #[must_use]
    pub fn allows(&self, cap: ToolCapability) -> bool {
        self.0.contains(&cap)
    }

    /// Empty scope — denies everything.
    #[must_use]
    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }
}

/// Phase-name-keyed lookup of the default capability scope.
///
/// Phases are named by their spec/project-loop command identifier
/// (e.g. `"do-task"`, `"audit-task"`). Returns `None` for unknown phases.
#[must_use]
pub fn default_scope_for_phase(phase: &str) -> Option<CapabilityScope> {
    use ToolCapability::{
        AdherenceRecord, ComplianceRecord, DemoFrontmatter, DemoResults, FeedbackReply, FindingAdd,
        IssueCreate, PhaseEscalate, PhaseOutcome, RubricRecord, SignpostAdd, SignpostUpdate,
        SpecFrontmatter, StandardRead, TaskAbandon, TaskComplete, TaskCreate, TaskRead, TaskRevise,
        TaskStart,
    };
    let caps: &[ToolCapability] = match phase {
        "shape-spec" => &[
            TaskCreate,
            TaskRevise,
            SpecFrontmatter,
            DemoFrontmatter,
            SignpostAdd,
            PhaseOutcome,
        ],
        "do-task" => &[
            TaskStart,
            TaskComplete,
            SignpostAdd,
            SignpostUpdate,
            TaskRead,
            PhaseOutcome,
        ],
        "audit-task" | "audit-spec" => &[
            FindingAdd,
            RubricRecord,
            ComplianceRecord,
            TaskRead,
            PhaseOutcome,
        ],
        "adhere-task" | "adhere-spec" => &[StandardRead, AdherenceRecord, TaskRead, PhaseOutcome],
        "run-demo" => &[DemoResults, FindingAdd, SignpostAdd, TaskRead, PhaseOutcome],
        "walk-spec" => &[TaskCreate, TaskRead, PhaseOutcome],
        "handle-feedback" => &[
            TaskCreate,
            IssueCreate,
            FeedbackReply,
            TaskRead,
            PhaseOutcome,
        ],
        "investigate" => &[
            TaskCreate,
            TaskRevise,
            TaskAbandon,
            FindingAdd,
            PhaseEscalate,
            TaskRead,
            PhaseOutcome,
        ],
        "resolve-blockers" => &[TaskCreate, TaskRevise, TaskAbandon, TaskRead, PhaseOutcome],
        "triage-audits" => &[IssueCreate, FindingAdd, PhaseOutcome],
        "sync-roadmap" => &[FindingAdd, PhaseOutcome],
        "discover-standards" | "index-standards" | "inject-standards" => {
            &[StandardRead, PhaseOutcome]
        }
        "plan-product" => &[PhaseOutcome],
        _ => return None,
    };
    Some(CapabilityScope::from_iter_caps(caps.iter().copied()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_snake_case_with_dot() {
        assert_eq!(ToolCapability::TaskCreate.tag(), "task.create");
        assert_eq!(ToolCapability::PhaseEscalate.tag(), "phase.escalate");
        assert_eq!(ToolCapability::StandardRead.tag(), "standard.read");
    }

    #[test]
    fn scope_allows() {
        let scope = CapabilityScope::from_iter_caps([
            ToolCapability::TaskRead,
            ToolCapability::PhaseOutcome,
        ]);
        assert!(scope.allows(ToolCapability::TaskRead));
        assert!(scope.allows(ToolCapability::PhaseOutcome));
        assert!(!scope.allows(ToolCapability::TaskCreate));
    }

    #[test]
    fn empty_scope_denies_all() {
        let scope = CapabilityScope::empty();
        assert!(!scope.allows(ToolCapability::TaskRead));
    }

    #[test]
    fn do_task_scope_matches_spec() {
        let scope = default_scope_for_phase("do-task").expect("do-task exists");
        assert!(scope.allows(ToolCapability::TaskStart));
        assert!(scope.allows(ToolCapability::TaskComplete));
        assert!(!scope.allows(ToolCapability::TaskCreate));
        assert!(!scope.allows(ToolCapability::PhaseEscalate));
    }

    #[test]
    fn investigate_is_the_only_phase_with_escalate() {
        for phase in [
            "shape-spec",
            "do-task",
            "audit-task",
            "adhere-task",
            "run-demo",
            "audit-spec",
            "adhere-spec",
            "walk-spec",
            "handle-feedback",
            "resolve-blockers",
            "triage-audits",
            "sync-roadmap",
            "discover-standards",
            "index-standards",
            "inject-standards",
            "plan-product",
        ] {
            let scope = default_scope_for_phase(phase).expect("known phase");
            assert!(
                !scope.allows(ToolCapability::PhaseEscalate),
                "phase {phase} unexpectedly has phase.escalate"
            );
        }
        let inv = default_scope_for_phase("investigate").expect("investigate");
        assert!(inv.allows(ToolCapability::PhaseEscalate));
    }

    #[test]
    fn feedback_reply_confined_to_handle_feedback_only() {
        for phase in [
            "shape-spec",
            "do-task",
            "audit-task",
            "adhere-task",
            "run-demo",
            "audit-spec",
            "adhere-spec",
            "walk-spec",
            "investigate",
            "resolve-blockers",
            "triage-audits",
            "discover-standards",
            "index-standards",
            "inject-standards",
            "plan-product",
        ] {
            let scope = default_scope_for_phase(phase).expect("known phase");
            assert!(
                !scope.allows(ToolCapability::FeedbackReply),
                "phase {phase} unexpectedly has feedback.reply"
            );
        }
        assert!(
            default_scope_for_phase("handle-feedback")
                .expect("handle-feedback")
                .allows(ToolCapability::FeedbackReply)
        );
        assert!(
            !default_scope_for_phase("sync-roadmap")
                .expect("sync-roadmap")
                .allows(ToolCapability::FeedbackReply)
        );
    }

    #[test]
    fn issue_create_confined_to_triage_and_feedback() {
        for phase in ["shape-spec", "do-task", "audit-task", "investigate"] {
            let scope = default_scope_for_phase(phase).expect("known phase");
            assert!(
                !scope.allows(ToolCapability::IssueCreate),
                "phase {phase} unexpectedly has issue.create"
            );
        }
        assert!(
            default_scope_for_phase("triage-audits")
                .expect("triage-audits")
                .allows(ToolCapability::IssueCreate)
        );
        assert!(
            default_scope_for_phase("handle-feedback")
                .expect("handle-feedback")
                .allows(ToolCapability::IssueCreate)
        );
    }

    #[test]
    fn unknown_phase_returns_none() {
        assert!(default_scope_for_phase("nonsense-phase").is_none());
    }

    #[test]
    fn scope_serde_roundtrip() {
        let scope = CapabilityScope::from_iter_caps([
            ToolCapability::TaskRead,
            ToolCapability::PhaseOutcome,
        ]);
        let json = serde_json::to_string(&scope).expect("serialize");
        let back: CapabilityScope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(scope, back);
    }
}
