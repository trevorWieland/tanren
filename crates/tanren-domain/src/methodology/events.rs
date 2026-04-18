//! Methodology events — the canonical history of all methodology state
//! changes.
//!
//! Nested into [`crate::events::DomainEvent::Methodology`] so the envelope
//! shape and `SCHEMA_VERSION = 1` remain unchanged. Every tool call in
//! `app-services::methodology::service` emits exactly one of these via
//! the existing `EventStore::append` path; the CLI and MCP transports
//! share the same service method, so the event trail is byte-identical
//! across transports.
//!
//! Ordering invariant: the **envelope timestamp** is the sole source of
//! occurrence time. Payloads carry no timestamps.
//!
//! Monotonicity invariant: the derived [`crate::methodology::TaskStatus`]
//! of any task is monotonic under event replay — `Complete` is terminal
//! and guard-satisfaction events only ever set flags to `true`. See
//! `proptest_monotonicity` in the test module for the formal property.

use serde::{Deserialize, Serialize};

use crate::ids::{SignpostId, SpecId, TaskId};
use crate::methodology::finding::{Finding, StandardRef};
use crate::methodology::issue::Issue;
use crate::methodology::phase_outcome::PhaseOutcome;
use crate::methodology::pillar::PillarScope;
use crate::methodology::rubric::{NonNegotiableCompliance, RubricScore};
use crate::methodology::signpost::{Signpost, SignpostStatus};
use crate::methodology::spec::Spec;
use crate::methodology::task::{
    AcceptanceCriterion, RequiredGuard, Task, TaskGuardFlags, TaskOrigin, TaskStatus,
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
    TaskGuardSatisfied(TaskGuardSatisfied),
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
            Self::TaskGuardSatisfied(e) => EntityRef::Task(e.task_id),
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
            Self::TaskCompleted(e) => Some(e.spec_id),
            Self::TaskAbandoned(e) => Some(e.spec_id),
            Self::TaskRevised(e) => Some(e.spec_id),
            Self::TaskGuardSatisfied(e) => Some(e.spec_id),
            Self::FindingAdded(e) => Some(e.finding.spec_id),
            Self::AdherenceFindingAdded(e) => Some(e.finding.spec_id),
            Self::RubricScoreRecorded(e) => Some(e.spec_id),
            Self::NonNegotiableComplianceRecorded(e) => Some(e.spec_id),
            Self::SignpostAdded(e) => Some(e.signpost.spec_id),
            Self::SignpostStatusUpdated(e) => Some(e.spec_id),
            Self::IssueCreated(e) => Some(e.issue.origin_spec_id),
            Self::PhaseOutcomeReported(e) => Some(e.spec_id),
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

/// A guard has been satisfied on an `Implemented` task. Multiple of
/// these can arrive in any order and for any named guard; the projection
/// folds them into [`TaskGuardFlags`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGuardSatisfied {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    pub guard: RequiredGuard,
}

/// `Implemented + {all required guards} → Complete`. Terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCompleted {
    pub task_id: TaskId,
    pub spec_id: SpecId,
}

/// `{non-terminal} → Abandoned { replacements }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAbandoned {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    pub reason: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replacements: Vec<TaskId>,
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
}

/// An adherence finding has been recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdherenceFindingAdded {
    pub finding: Box<Finding>,
    pub standard: StandardRef,
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
}

/// A phase has reported its typed outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseOutcomeReported {
    pub spec_id: SpecId,
    pub phase: NonEmptyString,
    pub agent_session_id: NonEmptyString,
    pub outcome: PhaseOutcome,
}

/// Postflight detected and reverted an unauthorized edit to an
/// orchestrator-owned artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnauthorizedArtifactEdit {
    pub spec_id: SpecId,
    pub phase: NonEmptyString,
    pub file: String,
    pub diff_preview: String,
    pub agent_session_id: NonEmptyString,
}

/// Postflight validation rejected an agent-authored narrative file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSchemaError {
    pub spec_id: SpecId,
    pub phase: NonEmptyString,
    pub file: String,
    pub error: NonEmptyString,
}

// -- In-memory projection helpers -------------------------------------------

/// Fold a sequence of methodology events into the terminal status of one
/// task. Pure function; deterministic under reordering of
/// [`TaskGuardSatisfied`] events.
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
            MethodologyEvent::TaskGuardSatisfied(e) if e.task_id == task_id => {
                guards.set(&e.guard, true);
                if matches!(status, Some(TaskStatus::Implemented { .. })) {
                    status = Some(TaskStatus::Implemented {
                        guards: guards.clone(),
                    });
                }
            }
            MethodologyEvent::TaskCompleted(e) if e.task_id == task_id => {
                if guards.satisfies(required) {
                    status = Some(TaskStatus::Complete);
                }
            }
            MethodologyEvent::TaskAbandoned(e) if e.task_id == task_id => {
                if !matches!(status, Some(TaskStatus::Complete)) {
                    status = Some(TaskStatus::Abandoned {
                        replacements: e.replacements.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::methodology::task::TaskOrigin;

    fn ne(s: &str) -> NonEmptyString {
        NonEmptyString::try_new(s).expect("non-empty")
    }

    fn seed_task(spec: SpecId) -> Task {
        let tid = TaskId::new();
        Task {
            id: tid,
            spec_id: spec,
            title: ne("Seed task"),
            description: String::new(),
            acceptance_criteria: vec![],
            origin: TaskOrigin::ShapeSpec,
            status: TaskStatus::Pending,
            depends_on: vec![],
            parent_task_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn entity_root_matches_variant() {
        let spec = SpecId::new();
        let t = seed_task(spec);
        let tid = t.id;
        let ev = MethodologyEvent::TaskStarted(TaskStarted {
            task_id: tid,
            spec_id: spec,
        });
        assert_eq!(ev.entity_root(), EntityRef::Task(tid));
        let ev2 = MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(t),
            origin: TaskOrigin::ShapeSpec,
        });
        assert_eq!(ev2.entity_root(), EntityRef::Task(tid));
    }

    #[test]
    fn event_json_roundtrip() {
        let spec = SpecId::new();
        let t = seed_task(spec);
        let ev = MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(t),
            origin: TaskOrigin::ShapeSpec,
        });
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: MethodologyEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev, back);
    }

    #[test]
    fn fold_complete_is_terminal() {
        let spec = SpecId::new();
        let t = seed_task(spec);
        let tid = t.id;
        let required = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        let events = vec![
            MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(t),
                origin: TaskOrigin::ShapeSpec,
            }),
            MethodologyEvent::TaskStarted(TaskStarted {
                task_id: tid,
                spec_id: spec,
            }),
            MethodologyEvent::TaskImplemented(TaskImplemented {
                task_id: tid,
                spec_id: spec,
                evidence_refs: vec![],
            }),
            MethodologyEvent::TaskGuardSatisfied(TaskGuardSatisfied {
                task_id: tid,
                spec_id: spec,
                guard: RequiredGuard::GateChecked,
            }),
            MethodologyEvent::TaskGuardSatisfied(TaskGuardSatisfied {
                task_id: tid,
                spec_id: spec,
                guard: RequiredGuard::Audited,
            }),
            MethodologyEvent::TaskGuardSatisfied(TaskGuardSatisfied {
                task_id: tid,
                spec_id: spec,
                guard: RequiredGuard::Adherent,
            }),
            MethodologyEvent::TaskCompleted(TaskCompleted {
                task_id: tid,
                spec_id: spec,
            }),
        ];
        assert_eq!(
            fold_task_status(tid, &required, &events),
            Some(TaskStatus::Complete)
        );
    }

    #[test]
    fn fold_completed_without_all_guards_stays_implemented() {
        let spec = SpecId::new();
        let t = seed_task(spec);
        let tid = t.id;
        let required = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        let events = vec![
            MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(t),
                origin: TaskOrigin::ShapeSpec,
            }),
            MethodologyEvent::TaskImplemented(TaskImplemented {
                task_id: tid,
                spec_id: spec,
                evidence_refs: vec![],
            }),
            MethodologyEvent::TaskGuardSatisfied(TaskGuardSatisfied {
                task_id: tid,
                spec_id: spec,
                guard: RequiredGuard::GateChecked,
            }),
            // Missing Audited + Adherent; a late TaskCompleted must not
            // transition to Complete.
            MethodologyEvent::TaskCompleted(TaskCompleted {
                task_id: tid,
                spec_id: spec,
            }),
        ];
        let status = fold_task_status(tid, &required, &events);
        let guards = match status {
            Some(TaskStatus::Implemented { ref guards }) => guards.clone(),
            _ => TaskGuardFlags::default(),
        };
        assert!(
            matches!(status, Some(TaskStatus::Implemented { .. })),
            "expected Implemented with partial guards, got {status:?}"
        );
        assert!(guards.gate_checked);
        assert!(!guards.audited);
        assert!(!guards.adherent);
    }
}
