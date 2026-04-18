//! Typed investigation-report.json document.
//!
//! Unlike the five markdown-with-frontmatter evidence files, the
//! investigation report is a pure JSON document emitted by
//! `investigate` before the phase exits. Structure per
//! `docs/architecture/evidence-schemas.md` §2.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{EventId, FindingId, SpecId, TaskId};
use crate::validated::NonEmptyString;

/// Typed investigation report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InvestigationReport {
    pub kind: InvestigationKind,
    pub spec_id: SpecId,
    pub investigation_id: EventId,
    pub trigger: InvestigationTrigger,
    #[serde(default)]
    pub root_causes: Vec<RootCause>,
    pub narrative: String,
    pub generated_at: DateTime<Utc>,
}

/// Fixed discriminant tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InvestigationKind {
    Investigation,
}

/// What caused the investigation to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InvestigationTrigger {
    pub phase: NonEmptyString,
    pub task_id: Option<TaskId>,
    pub error_fingerprint: NonEmptyString,
    pub loop_index: u16,
}

/// One proposed root cause with its remediation actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RootCause {
    pub description: NonEmptyString,
    pub confidence: Confidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
    pub category: RootCauseCategory,
    #[serde(default)]
    pub suggested_actions: Vec<SuggestedAction>,
}

/// Confidence level attached to a root-cause hypothesis.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// High-level taxonomy of root-cause categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RootCauseCategory {
    CodeBug,
    SpecAmbiguity,
    AcceptanceCriteriaGap,
    EnvironmentDrift,
    TestGap,
    ExternalDependency,
}

/// Typed remediation action the orchestrator can enact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SuggestedAction {
    ReviseTask {
        task_id: TaskId,
        reason: NonEmptyString,
    },
    CreateTask {
        title: NonEmptyString,
        description: String,
        origin_detail: NonEmptyString,
    },
    /// Mark an existing finding as the one to carry forward as the
    /// root-cause trace.
    LinkFinding {
        finding_id: FindingId,
    },
    Escalate {
        reason: NonEmptyString,
    },
}

impl InvestigationReport {
    /// Parse a JSON report.
    ///
    /// # Errors
    /// Returns [`serde_json::Error`] on malformed input.
    pub fn parse_from_json(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }

    /// Render as canonical pretty-printed JSON with a trailing newline.
    ///
    /// # Errors
    /// Returns [`serde_json::Error`] if self fails to serialize.
    pub fn render_to_json(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string_pretty(self)?;
        s.push('\n');
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ne(s: &str) -> NonEmptyString {
        NonEmptyString::try_new(s).expect("non-empty")
    }

    #[test]
    fn roundtrip_stable() {
        let r = InvestigationReport {
            kind: InvestigationKind::Investigation,
            spec_id: SpecId::new(),
            investigation_id: EventId::new(),
            trigger: InvestigationTrigger {
                phase: ne("do-task"),
                task_id: Some(TaskId::new()),
                error_fingerprint: ne("abc123"),
                loop_index: 1,
            },
            root_causes: vec![RootCause {
                description: ne("null deref in X"),
                confidence: Confidence::High,
                affected_files: vec!["src/x.rs".into()],
                category: RootCauseCategory::CodeBug,
                suggested_actions: vec![SuggestedAction::ReviseTask {
                    task_id: TaskId::new(),
                    reason: ne("clarify acceptance"),
                }],
            }],
            narrative: "Investigation found a null-deref in X".into(),
            generated_at: Utc::now(),
        };
        let json = r.render_to_json().expect("render");
        let back = InvestigationReport::parse_from_json(&json).expect("parse");
        assert_eq!(r, back);
    }

    #[test]
    fn suggested_action_tagged_representation() {
        let a = SuggestedAction::Escalate {
            reason: ne("loop-cap"),
        };
        let v = serde_json::to_value(&a).expect("serialize");
        assert_eq!(v["kind"], "escalate");
        assert_eq!(v["reason"], "loop-cap");
    }
}
