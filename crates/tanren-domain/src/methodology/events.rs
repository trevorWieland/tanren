//! Methodology events — the canonical history of methodology state.
//!
//! Nested into [`crate::events::DomainEvent::Methodology`] so the
//! envelope shape and `SCHEMA_VERSION = 1` remain unchanged. Both
//! CLI and MCP transports emit these via the shared service; the
//! trail is byte-identical across transports. `Complete` is terminal;
//! guard flags are monotonic under replay.

use serde::{Deserialize, Serialize};

use crate::ids::{SignpostId, SpecId, TaskId};
use crate::methodology::finding::{Finding, StandardRef};
use crate::methodology::issue::Issue;
use crate::methodology::phase_id::PhaseId;
use crate::methodology::phase_outcome::PhaseOutcome;
use crate::methodology::pillar::PillarScope;
use crate::methodology::rubric::{NonNegotiableCompliance, RubricScore};
use crate::methodology::signpost::{Signpost, SignpostStatus};
use crate::methodology::spec::Spec;
use crate::methodology::task::{
    AcceptanceCriterion, ExplicitUserDiscardProvenance, RequiredGuard, Task,
    TaskAbandonDisposition, TaskGuardFlags, TaskOrigin, TaskStatus,
};
use crate::validated::NonEmptyString;

use crate::entity::EntityRef;

/// All methodology events. Nested under
/// [`crate::events::DomainEvent::Methodology`] so adding variants here
/// never bumps [`crate::events::SCHEMA_VERSION`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum MethodologyEvent {
    SpecDefined(SpecDefined),
    TaskCreated(TaskCreated),
    TaskStarted(TaskStarted),
    TaskImplemented(TaskImplemented),
    TaskGateChecked(TaskGateChecked),
    TaskAudited(TaskAudited),
    TaskAdherent(TaskAdherent),
    TaskXChecked(TaskXChecked),
    TaskCompleted(TaskCompleted),
    TaskAbandoned(TaskAbandoned),
    TaskRevised(TaskRevised),
    FindingAdded(FindingAdded),
    AdherenceFindingAdded(AdherenceFindingAdded),
    RubricScoreRecorded(RubricScoreRecorded),
    NonNegotiableComplianceRecorded(NonNegotiableComplianceRecorded),
    SignpostAdded(SignpostAdded),
    SignpostStatusUpdated(SignpostStatusUpdated),
    IssueCreated(IssueCreated),
    PhaseOutcomeReported(PhaseOutcomeReported),
    ReplyDirectiveRecorded(ReplyDirectiveRecorded),
    SpecFrontmatterUpdated(SpecFrontmatterUpdated),
    DemoFrontmatterUpdated(DemoFrontmatterUpdated),
    UnauthorizedArtifactEdit(UnauthorizedArtifactEdit),
    EvidenceSchemaError(EvidenceSchemaError),
}

impl MethodologyEvent {
    /// Return the typed root [`EntityRef`] this event correlates to.
    #[must_use]
    pub fn entity_root(&self) -> EntityRef {
        match self {
            Self::SpecDefined(e) => EntityRef::Spec(e.spec.id),
            Self::TaskCreated(e) => EntityRef::Task(e.task.id),
            Self::TaskStarted(e) => EntityRef::Task(e.task_id),
            Self::TaskImplemented(e) => EntityRef::Task(e.task_id),
            Self::TaskGateChecked(e) => EntityRef::Task(e.task_id),
            Self::TaskAudited(e) => EntityRef::Task(e.task_id),
            Self::TaskAdherent(e) => EntityRef::Task(e.task_id),
            Self::TaskXChecked(e) => EntityRef::Task(e.task_id),
            Self::TaskCompleted(e) => EntityRef::Task(e.task_id),
            Self::TaskAbandoned(e) => EntityRef::Task(e.task_id),
            Self::TaskRevised(e) => EntityRef::Task(e.task_id),
            Self::FindingAdded(e) => EntityRef::Finding(e.finding.id),
            Self::AdherenceFindingAdded(e) => EntityRef::Finding(e.finding.id),
            Self::RubricScoreRecorded(e) => EntityRef::Spec(e.spec_id),
            Self::NonNegotiableComplianceRecorded(e) => EntityRef::Spec(e.spec_id),
            Self::SignpostAdded(e) => EntityRef::Signpost(e.signpost.id),
            Self::SignpostStatusUpdated(e) => EntityRef::Signpost(e.signpost_id),
            Self::IssueCreated(e) => EntityRef::Issue(e.issue.id),
            Self::PhaseOutcomeReported(e) => EntityRef::Spec(e.spec_id),
            Self::ReplyDirectiveRecorded(e) => EntityRef::Spec(e.spec_id),
            Self::SpecFrontmatterUpdated(e) => EntityRef::Spec(e.spec_id),
            Self::DemoFrontmatterUpdated(e) => EntityRef::Spec(e.spec_id),
            Self::UnauthorizedArtifactEdit(e) => EntityRef::Spec(e.spec_id),
            Self::EvidenceSchemaError(e) => EntityRef::Spec(e.spec_id),
        }
    }

    /// Spec id this event correlates to, if any. Used by projection
    /// functions to scope event scans.
    #[must_use]
    pub fn spec_id(&self) -> Option<SpecId> {
        match self {
            Self::SpecDefined(e) => Some(e.spec.id),
            Self::TaskCreated(e) => Some(e.task.spec_id),
            Self::TaskStarted(e) => Some(e.spec_id),
            Self::TaskImplemented(e) => Some(e.spec_id),
            Self::TaskGateChecked(e) => Some(e.spec_id),
            Self::TaskAudited(e) => Some(e.spec_id),
            Self::TaskAdherent(e) => Some(e.spec_id),
            Self::TaskXChecked(e) => Some(e.spec_id),
            Self::TaskCompleted(e) => Some(e.spec_id),
            Self::TaskAbandoned(e) => Some(e.spec_id),
            Self::TaskRevised(e) => Some(e.spec_id),
            Self::FindingAdded(e) => Some(e.finding.spec_id),
            Self::AdherenceFindingAdded(e) => Some(e.finding.spec_id),
            Self::RubricScoreRecorded(e) => Some(e.spec_id),
            Self::NonNegotiableComplianceRecorded(e) => Some(e.spec_id),
            Self::SignpostAdded(e) => Some(e.signpost.spec_id),
            Self::SignpostStatusUpdated(e) => Some(e.spec_id),
            Self::IssueCreated(e) => Some(e.issue.origin_spec_id),
            Self::PhaseOutcomeReported(e) => Some(e.spec_id),
            Self::ReplyDirectiveRecorded(e) => Some(e.spec_id),
            Self::SpecFrontmatterUpdated(e) => Some(e.spec_id),
            Self::DemoFrontmatterUpdated(e) => Some(e.spec_id),
            Self::UnauthorizedArtifactEdit(e) => Some(e.spec_id),
            Self::EvidenceSchemaError(e) => Some(e.spec_id),
        }
    }
}

// -- Per-event payload structs -----------------------------------------------

/// A new spec has been opened. Emitted exactly once per spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecDefined {
    pub spec: Box<Spec>,
}

/// A new task has been created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreated {
    pub task: Box<Task>,
    pub origin: TaskOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `Pending → InProgress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStarted {
    pub task_id: TaskId,
    pub spec_id: SpecId,
}

/// `InProgress → Implemented { guards: Default }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskImplemented {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// `Implemented` task has passed the task-gate check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGateChecked {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `Implemented` task has passed task-scoped audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAudited {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `Implemented` task has passed task-scoped adherence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAdherent {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `Implemented` task has passed an extra, config-defined guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskXChecked {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    pub guard_name: NonEmptyString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `Implemented + {all required guards} → Complete`. Terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCompleted {
    pub task_id: TaskId,
    pub spec_id: SpecId,
}

/// `{non-terminal} → Abandoned` with typed disposition/provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAbandoned {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    pub reason: NonEmptyString,
    #[serde(default)]
    pub disposition: TaskAbandonDisposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replacements: Vec<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit_user_discard_provenance: Option<ExplicitUserDiscardProvenance>,
}

/// A non-transitional revision of a task's description / acceptance.
/// Does **not** change `TaskStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRevised {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    pub revised_description: String,
    pub revised_acceptance: Vec<AcceptanceCriterion>,
    pub reason: NonEmptyString,
}

/// An audit or demo finding has been recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingAdded {
    pub finding: Box<Finding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// An adherence finding has been recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdherenceFindingAdded {
    pub finding: Box<Finding>,
    pub standard: StandardRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// One rubric score has been recorded on an audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricScoreRecorded {
    pub spec_id: SpecId,
    pub scope: PillarScope,
    pub scope_target_id: Option<String>,
    pub score: RubricScore,
}

/// A non-negotiable check has been recorded on an audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonNegotiableComplianceRecorded {
    pub spec_id: SpecId,
    pub scope: PillarScope,
    pub compliance: NonNegotiableCompliance,
}

/// A signpost has been added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignpostAdded {
    pub signpost: Box<Signpost>,
}

/// A signpost's status has been updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignpostStatusUpdated {
    pub signpost_id: SignpostId,
    pub spec_id: SpecId,
    pub status: SignpostStatus,
    pub resolution: Option<String>,
}

/// A backlog issue has been created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueCreated {
    pub issue: Box<Issue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A phase has reported its typed outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseOutcomeReported {
    pub spec_id: SpecId,
    pub phase: PhaseId,
    pub agent_session_id: NonEmptyString,
    pub outcome: PhaseOutcome,
}

pub use crate::methodology::frontmatter_patch::{
    DemoFrontmatterPatch, DemoFrontmatterUpdated, SpecFrontmatterPatch, SpecFrontmatterUpdated,
};

/// A `handle-feedback` reply directive. Orchestrator enacts the post.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyDirectiveRecorded {
    pub spec_id: SpecId,
    pub phase: PhaseId,
    pub thread_ref: NonEmptyString,
    pub disposition: crate::methodology::phase_outcome::ReplyDisposition,
    pub body: String,
}

/// Postflight detected and reverted an unauthorized edit to an
/// orchestrator-owned artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnauthorizedArtifactEdit {
    pub spec_id: SpecId,
    pub phase: PhaseId,
    pub file: String,
    pub diff_preview: String,
    pub agent_session_id: NonEmptyString,
}

/// Postflight validation rejected an agent-authored narrative file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSchemaError {
    pub spec_id: SpecId,
    pub phase: PhaseId,
    pub file: String,
    pub error: NonEmptyString,
}

// -- In-memory projection helpers -------------------------------------------

/// Fold a sequence of methodology events into the terminal status of one
/// task. Pure function; deterministic under reordering of
/// guard-check events.
///
/// `required` controls which guards must be satisfied for the task to
/// converge to [`TaskStatus::Complete`] after `TaskCompleted` arrives.
/// Matches the config-driven `task_complete_requires` list.
#[must_use]
pub fn fold_task_status<'a, I>(
    task_id: TaskId,
    required: &[RequiredGuard],
    events: I,
) -> Option<TaskStatus>
where
    I: IntoIterator<Item = &'a MethodologyEvent>,
{
    let mut status: Option<TaskStatus> = None;
    let mut guards = TaskGuardFlags::default();
    for ev in events {
        match ev {
            MethodologyEvent::TaskCreated(e) if e.task.id == task_id => {
                status = Some(TaskStatus::Pending);
            }
            MethodologyEvent::TaskStarted(e) if e.task_id == task_id => {
                if !matches!(
                    status,
                    Some(TaskStatus::Complete | TaskStatus::Abandoned { .. })
                ) {
                    status = Some(TaskStatus::InProgress);
                }
            }
            MethodologyEvent::TaskImplemented(e) if e.task_id == task_id => {
                if !matches!(
                    status,
                    Some(TaskStatus::Complete | TaskStatus::Abandoned { .. })
                ) {
                    status = Some(TaskStatus::Implemented {
                        guards: guards.clone(),
                    });
                }
            }
            MethodologyEvent::TaskGateChecked(e) if e.task_id == task_id => {
                guards.set(&RequiredGuard::GateChecked, true);
                if matches!(status, Some(TaskStatus::Implemented { .. })) {
                    status = Some(TaskStatus::Implemented {
                        guards: guards.clone(),
                    });
                }
            }
            MethodologyEvent::TaskAudited(e) if e.task_id == task_id => {
                guards.set(&RequiredGuard::Audited, true);
                if matches!(status, Some(TaskStatus::Implemented { .. })) {
                    status = Some(TaskStatus::Implemented {
                        guards: guards.clone(),
                    });
                }
            }
            MethodologyEvent::TaskAdherent(e) if e.task_id == task_id => {
                guards.set(&RequiredGuard::Adherent, true);
                if matches!(status, Some(TaskStatus::Implemented { .. })) {
                    status = Some(TaskStatus::Implemented {
                        guards: guards.clone(),
                    });
                }
            }
            MethodologyEvent::TaskXChecked(e) if e.task_id == task_id => {
                guards.set(
                    &RequiredGuard::Extra(e.guard_name.as_str().to_owned()),
                    true,
                );
                if matches!(status, Some(TaskStatus::Implemented { .. })) {
                    status = Some(TaskStatus::Implemented {
                        guards: guards.clone(),
                    });
                }
            }
            MethodologyEvent::TaskCompleted(e) if e.task_id == task_id => {
                if matches!(status, Some(TaskStatus::Implemented { .. }))
                    && guards.satisfies(required)
                {
                    status = Some(TaskStatus::Complete);
                }
            }
            MethodologyEvent::TaskAbandoned(e) if e.task_id == task_id => {
                if !matches!(status, Some(TaskStatus::Complete)) {
                    status = Some(TaskStatus::Abandoned {
                        disposition: e.disposition,
                        replacements: e.replacements.clone(),
                        explicit_user_discard_provenance: e
                            .explicit_user_discard_provenance
                            .clone(),
                    });
                }
            }
            _ => {}
        }
    }
    status
}
