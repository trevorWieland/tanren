//! Typed task lifecycle.
//!
//! A [`Task`] is the unit of planned work inside a spec. Its [`TaskStatus`]
//! is monotonic: `Complete` is terminal and cannot be reopened. Remediation
//! is always a new task, never a re-transition.
//!
//! # Multi-guard completion
//!
//! A task moves `Pending → InProgress → Implemented`, and then collects
//! one or more completion guards in parallel (gate-checked, audited,
//! adherent, plus any extensible `x_checked` guards configured for the
//! spec). Only when every *required* guard is satisfied does the task
//! transition to `Complete`.
//!
//! See `docs/architecture/orchestration-flow.md` §2.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{FindingId, SpecId, TaskId};
use crate::methodology::phase_id::PhaseId;
use crate::validated::NonEmptyString;

/// Typed abandonment disposition for task terminalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskAbandonDisposition {
    #[default]
    Replacement,
    ExplicitUserDiscard,
}

/// Provenance required when a task is abandoned via explicit user discard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExplicitUserDiscardProvenance {
    ResolveBlockers { resolution_note: NonEmptyString },
}

/// Lifecycle state of a task, including which completion guards have
/// been satisfied when the task is `Implemented`.
///
/// `Complete` is **terminal**: no transition out is legal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TaskStatus {
    /// Created, not yet started.
    Pending,
    /// An implementing phase is actively executing the task.
    InProgress,
    /// Implementation finished; guards now gate the `Complete` transition.
    Implemented {
        /// Per-guard satisfaction flags. Derived from observed events;
        /// never set directly by API consumers.
        guards: TaskGuardFlags,
    },
    /// All required guards satisfied; terminal.
    Complete,
    /// Abandoned side-branch; replacement task(s) may reference this one.
    Abandoned {
        #[serde(default)]
        disposition: TaskAbandonDisposition,
        /// Tasks created to replace this one, if any.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        replacements: Vec<TaskId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        explicit_user_discard_provenance: Option<ExplicitUserDiscardProvenance>,
    },
}

impl TaskStatus {
    /// Return true iff this state is terminal (no outbound transition).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Abandoned { .. })
    }

    /// Short human-readable tag, used by logging and display.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Implemented { .. } => "implemented",
            Self::Complete => "complete",
            Self::Abandoned { .. } => "abandoned",
        }
    }

    /// Check whether the given transition event is legal from this
    /// state. Returns `Ok(LegalTransition::Transition)` on a legal
    /// advance, `Ok(LegalTransition::Idempotent)` when the transition
    /// is a no-op on the current state (e.g. `start_task` on an
    /// `InProgress` task), and `Err(IllegalTransition)` otherwise.
    ///
    /// Terminal states (`Complete`, `Abandoned`) reject every transition.
    /// Call-level idempotency is provided by the methodology idempotency
    /// reservation layer, not by allowing terminal-state re-transitions.
    ///
    /// # Errors
    /// Returns [`IllegalTransition`] when the attempted event is not
    /// legal from `self`.
    pub fn legal_next(
        &self,
        event: TaskTransitionKind,
    ) -> Result<LegalTransition, IllegalTransition> {
        use TaskTransitionKind as K;
        match (self, event) {
            // Legal advances + idempotent no-ops.
            (Self::Pending, K::Start)
            | (Self::InProgress, K::Implement)
            | (Self::Implemented { .. }, K::Guard | K::Complete)
            | (
                Self::Pending | Self::InProgress | Self::Implemented { .. },
                K::Revise | K::Abandon,
            ) => Ok(LegalTransition::Transition),
            (Self::InProgress, K::Start) | (Self::Implemented { .. }, K::Implement) => {
                Ok(LegalTransition::Idempotent)
            }
            // Terminal states and every other combination are illegal.
            _ => Err(IllegalTransition {
                from: self.tag(),
                attempted: event.tag(),
            }),
        }
    }
}

/// Classes of state-machine events the task may experience. Maps 1:1
/// to `MethodologyEvent` variants that touch task state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTransitionKind {
    Start,
    Implement,
    Guard,
    Complete,
    Revise,
    Abandon,
}

impl TaskTransitionKind {
    /// Stable string tag used by errors and diagnostic output.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Implement => "implement",
            Self::Guard => "guard",
            Self::Complete => "complete",
            Self::Revise => "revise",
            Self::Abandon => "abandon",
        }
    }
}

/// Outcome of a [`TaskStatus::legal_next`] check that *is* legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalTransition {
    /// The event moves the task to a new state.
    Transition,
    /// The event is redundant at the current state and should be
    /// treated as a no-op by the caller (content-idempotent).
    Idempotent,
}

/// Error returned by [`TaskStatus::legal_next`] for illegal advances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: &'static str,
    pub attempted: &'static str,
}

impl std::fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "illegal task transition: {} → {}",
            self.from, self.attempted
        )
    }
}

impl std::error::Error for IllegalTransition {}

/// Per-guard satisfaction flags recorded on the `Implemented` state.
///
/// Each flag corresponds to one independent guard phase (gate / audit /
/// adherence / extensible). Guards execute in parallel and can arrive in
/// any order; the task transitions to `Complete` only when every required
/// guard (as configured in `tanren.yml`'s `task_complete_requires`) is
/// set to `true`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "snake_case")]
pub struct TaskGuardFlags {
    /// Automated task-gate (e.g. `just check`) passed.
    pub gate_checked: bool,
    /// Audit phase produced zero `fix_now` findings scoped to this task.
    pub audited: bool,
    /// Adherence phase produced zero `fix_now` findings scoped to this task.
    pub adherent: bool,
    /// Extensible, config-defined guards. Keys match guard names in config.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extra: std::collections::BTreeMap<String, bool>,
}

impl TaskGuardFlags {
    /// Return true iff every required guard is satisfied.
    #[must_use]
    pub fn satisfies(&self, required: &[RequiredGuard]) -> bool {
        required.iter().all(|g| self.get(g))
    }

    /// Read a guard flag by name.
    #[must_use]
    pub fn get(&self, guard: &RequiredGuard) -> bool {
        match guard {
            RequiredGuard::GateChecked => self.gate_checked,
            RequiredGuard::Audited => self.audited,
            RequiredGuard::Adherent => self.adherent,
            RequiredGuard::Extra(name) => self.extra.get(name).copied().unwrap_or(false),
        }
    }

    /// Set a guard flag by name.
    pub fn set(&mut self, guard: &RequiredGuard, value: bool) {
        match guard {
            RequiredGuard::GateChecked => self.gate_checked = value,
            RequiredGuard::Audited => self.audited = value,
            RequiredGuard::Adherent => self.adherent = value,
            RequiredGuard::Extra(name) => {
                self.extra.insert(name.clone(), value);
            }
        }
    }
}

/// A required guard, either one of the three built-ins or a config-named
/// extensible guard.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequiredGuard {
    GateChecked,
    Audited,
    Adherent,
    /// Extensible guard named in config.
    Extra(String),
}

impl std::fmt::Display for RequiredGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GateChecked => f.write_str("gate_checked"),
            Self::Audited => f.write_str("audited"),
            Self::Adherent => f.write_str("adherent"),
            Self::Extra(s) => f.write_str(s),
        }
    }
}

/// Provenance of a task — what caused it to be created.
///
/// Matches `orchestration-flow.md` §2.3. Used to preserve traceability
/// across the spec-loop (a task created from an audit finding remembers
/// *which finding* created it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskOrigin {
    /// Authored during initial spec shaping.
    ShapeSpec,
    /// Emitted by `investigate` during root-cause diagnosis.
    Investigation {
        source_phase: PhaseId,
        source_task: Option<TaskId>,
        loop_index: u16,
    },
    /// Remediation for an audit finding at task scope.
    Audit {
        source_phase: PhaseId,
        source_task: Option<TaskId>,
        source_finding: FindingId,
    },
    /// Remediation for an adherence violation.
    Adherence {
        source_standard: NonEmptyString,
        source_finding: FindingId,
    },
    /// Recovery from a failed demo step.
    Demo {
        source_run: NonEmptyString,
        source_finding: FindingId,
    },
    /// Follow-up from reviewer feedback on a PR.
    Feedback {
        source_pr_comment_ref: NonEmptyString,
    },
    /// Remediation for an audit finding at spec scope.
    SpecAudit { source_finding: FindingId },
    /// Diagnosis from `investigate` when acting at spec scope.
    SpecInvestigation {
        source_phase: PhaseId,
        source_finding: FindingId,
    },
    /// Work needed in this spec surfaced from work in another spec.
    CrossSpecIntent {
        source_spec_id: SpecId,
        source_finding: FindingId,
    },
    /// Merge-in work from another spec explicitly requested by the user.
    CrossSpecMerge { source_spec_id: SpecId },
    /// User-authored, ad-hoc origin.
    User,
}

/// Canonical task record.
///
/// This is a projection: stores derive it by folding the event log. Tools
/// never mutate a `Task` in place; they emit typed methodology events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Task {
    pub id: TaskId,
    pub spec_id: SpecId,
    pub title: NonEmptyString,
    pub description: String,
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    pub origin: TaskOrigin,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskId>,
    pub parent_task_id: Option<TaskId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One acceptance criterion on a task or spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AcceptanceCriterion {
    pub id: NonEmptyString,
    pub description: NonEmptyString,
    pub measurable: NonEmptyString,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_are_terminal() {
        assert!(TaskStatus::Complete.is_terminal());
        assert!(
            TaskStatus::Abandoned {
                disposition: TaskAbandonDisposition::Replacement,
                replacements: vec![],
                explicit_user_discard_provenance: None
            }
            .is_terminal()
        );
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::InProgress.is_terminal());
        assert!(
            !TaskStatus::Implemented {
                guards: TaskGuardFlags::default()
            }
            .is_terminal()
        );
    }

    #[test]
    fn guard_flags_satisfy_required() {
        let mut flags = TaskGuardFlags::default();
        let req = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        assert!(!flags.satisfies(&req));
        flags.gate_checked = true;
        flags.audited = true;
        flags.adherent = true;
        assert!(flags.satisfies(&req));
    }

    #[test]
    fn guard_flags_extra() {
        let mut flags = TaskGuardFlags::default();
        let req = [RequiredGuard::Extra("security_reviewed".into())];
        assert!(!flags.satisfies(&req));
        flags.set(&RequiredGuard::Extra("security_reviewed".into()), true);
        assert!(flags.satisfies(&req));
    }

    #[test]
    fn task_status_tag_is_stable() {
        assert_eq!(TaskStatus::Pending.tag(), "pending");
        assert_eq!(TaskStatus::InProgress.tag(), "in_progress");
        assert_eq!(
            TaskStatus::Implemented {
                guards: TaskGuardFlags::default()
            }
            .tag(),
            "implemented"
        );
        assert_eq!(TaskStatus::Complete.tag(), "complete");
        assert_eq!(
            TaskStatus::Abandoned {
                disposition: TaskAbandonDisposition::Replacement,
                replacements: vec![],
                explicit_user_discard_provenance: None
            }
            .tag(),
            "abandoned"
        );
    }

    #[test]
    fn task_origin_serde_roundtrip() {
        let origin = TaskOrigin::Audit {
            source_phase: PhaseId::try_new("audit-task").expect("phase"),
            source_task: Some(TaskId::new()),
            source_finding: FindingId::new(),
        };
        let json = serde_json::to_string(&origin).expect("serialize");
        let back: TaskOrigin = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(origin, back);
    }

    #[test]
    fn required_guard_display() {
        assert_eq!(RequiredGuard::GateChecked.to_string(), "gate_checked");
        assert_eq!(RequiredGuard::Audited.to_string(), "audited");
        assert_eq!(RequiredGuard::Adherent.to_string(), "adherent");
        assert_eq!(
            RequiredGuard::Extra("security_reviewed".into()).to_string(),
            "security_reviewed"
        );
    }

    #[test]
    fn explicit_user_discard_provenance_roundtrip() {
        let provenance = ExplicitUserDiscardProvenance::ResolveBlockers {
            resolution_note: NonEmptyString::try_new("user chose to discard path").expect("note"),
        };
        let json = serde_json::to_string(&provenance).expect("serialize");
        let back: ExplicitUserDiscardProvenance = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, provenance);
    }
}
