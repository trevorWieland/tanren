//! Shared event ↔ tool attribution rules.
//!
//! Used by the projector, replay validator, and tests to keep tool-name
//! attribution consistent in one source of truth.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::methodology::events::{DemoFrontmatterPatch, MethodologyEvent, SpecFrontmatterPatch};

/// Event origin classification in `phase-events.jsonl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PhaseEventOriginKind {
    /// Directly emitted by the called tool.
    ToolPrimary,
    /// Secondary event emitted within the same tool call.
    ToolDerived,
    /// Emitted by system/session postflight code.
    System,
}

impl PhaseEventOriginKind {
    /// Default origin for an event when no explicit call context is present.
    #[must_use]
    pub const fn default_for_event(event: &MethodologyEvent) -> Self {
        match event {
            MethodologyEvent::UnauthorizedArtifactEdit(_)
            | MethodologyEvent::EvidenceSchemaError(_) => Self::System,
            _ => Self::ToolPrimary,
        }
    }
}

/// Canonical tool label for an event (new writes).
#[must_use]
pub fn canonical_tool_for_event(event: &MethodologyEvent) -> &'static str {
    match event {
        MethodologyEvent::TaskGateChecked(_)
        | MethodologyEvent::TaskAudited(_)
        | MethodologyEvent::TaskAdherent(_)
        | MethodologyEvent::TaskXChecked(_)
        | MethodologyEvent::TaskCompleted(_) => "mark_task_guard_satisfied",
        MethodologyEvent::SpecFrontmatterUpdated(e) => match &e.patch {
            SpecFrontmatterPatch::SetTitle { .. } => "set_spec_title",
            SpecFrontmatterPatch::SetNonNegotiables { .. } => "set_spec_non_negotiables",
            SpecFrontmatterPatch::AddAcceptanceCriterion { .. } => "add_spec_acceptance_criterion",
            SpecFrontmatterPatch::SetDemoEnvironment { .. } => "set_spec_demo_environment",
            SpecFrontmatterPatch::SetDependencies { .. } => "set_spec_dependencies",
            SpecFrontmatterPatch::SetBaseBranch { .. } => "set_spec_base_branch",
            SpecFrontmatterPatch::SetRelevanceContext { .. } => "set_spec_relevance_context",
        },
        MethodologyEvent::DemoFrontmatterUpdated(e) => match &e.patch {
            DemoFrontmatterPatch::AddStep { .. } => "add_demo_step",
            DemoFrontmatterPatch::MarkStepSkip { .. } => "mark_demo_step_skip",
            DemoFrontmatterPatch::AppendResult { .. } => "append_demo_result",
        },
        MethodologyEvent::UnauthorizedArtifactEdit(_)
        | MethodologyEvent::EvidenceSchemaError(_) => "finalize_mutation_session",
        _ => allowed_tools_for_event(event)[0],
    }
}

/// Allowed tool labels for one event variant.
///
/// Includes legacy sentinel aliases for replay backward compatibility.
#[must_use]
pub fn allowed_tools_for_event(event: &MethodologyEvent) -> &'static [&'static str] {
    match event {
        MethodologyEvent::SpecDefined(_) => &["shape-spec"],
        MethodologyEvent::TaskCreated(_) => &["create_task"],
        MethodologyEvent::TaskStarted(_) => &["start_task"],
        MethodologyEvent::TaskImplemented(_) => &["complete_task"],
        MethodologyEvent::TaskGateChecked(_)
        | MethodologyEvent::TaskAudited(_)
        | MethodologyEvent::TaskAdherent(_)
        | MethodologyEvent::TaskXChecked(_) => &["mark_task_guard_satisfied", "<guard-phase>"],
        MethodologyEvent::TaskCompleted(_) => &[
            "mark_task_guard_satisfied",
            "complete_task",
            "<orchestrator>",
        ],
        MethodologyEvent::TaskAbandoned(_) => &["abandon_task"],
        MethodologyEvent::TaskRevised(_) => &["revise_task"],
        MethodologyEvent::FindingAdded(_) => &["add_finding"],
        MethodologyEvent::AdherenceFindingAdded(_) => &["record_adherence_finding"],
        MethodologyEvent::RubricScoreRecorded(_) => &["record_rubric_score"],
        MethodologyEvent::NonNegotiableComplianceRecorded(_) => {
            &["record_non_negotiable_compliance"]
        }
        MethodologyEvent::SignpostAdded(_) => &["add_signpost"],
        MethodologyEvent::SignpostStatusUpdated(_) => &["update_signpost_status"],
        MethodologyEvent::IssueCreated(_) => &["create_issue"],
        MethodologyEvent::PhaseOutcomeReported(_) => {
            &["report_phase_outcome", "escalate_to_blocker"]
        }
        MethodologyEvent::ReplyDirectiveRecorded(_) => &["post_reply_directive"],
        MethodologyEvent::SpecFrontmatterUpdated(e) => match &e.patch {
            SpecFrontmatterPatch::SetTitle { .. } => &["set_spec_title"],
            SpecFrontmatterPatch::SetNonNegotiables { .. } => &["set_spec_non_negotiables"],
            SpecFrontmatterPatch::AddAcceptanceCriterion { .. } => {
                &["add_spec_acceptance_criterion"]
            }
            SpecFrontmatterPatch::SetDemoEnvironment { .. } => &["set_spec_demo_environment"],
            SpecFrontmatterPatch::SetDependencies { .. } => &["set_spec_dependencies"],
            SpecFrontmatterPatch::SetBaseBranch { .. } => &["set_spec_base_branch"],
            SpecFrontmatterPatch::SetRelevanceContext { .. } => &["set_spec_relevance_context"],
        },
        MethodologyEvent::DemoFrontmatterUpdated(e) => match &e.patch {
            DemoFrontmatterPatch::AddStep { .. } => &["add_demo_step"],
            DemoFrontmatterPatch::MarkStepSkip { .. } => &["mark_demo_step_skip"],
            DemoFrontmatterPatch::AppendResult { .. } => &["append_demo_result"],
        },
        MethodologyEvent::UnauthorizedArtifactEdit(_) => {
            &["finalize_mutation_session", "<enforcement>"]
        }
        MethodologyEvent::EvidenceSchemaError(_) => &["finalize_mutation_session", "<postflight>"],
    }
}

/// True when `tool` is allowed for `event`.
#[must_use]
pub fn is_tool_allowed_for_event(event: &MethodologyEvent, tool: &str) -> bool {
    allowed_tools_for_event(event).contains(&tool)
}
