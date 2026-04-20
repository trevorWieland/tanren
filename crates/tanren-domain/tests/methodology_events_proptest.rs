//! Property tests for methodology event monotonicity and guard
//! composition.
//!
//! Non-negotiable #1: task state is monotonic. `Complete` is terminal.
//! Guards satisfy in any order and converge to the same final state.

use chrono::Utc;
use proptest::prelude::*;

use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAbandoned, TaskAdherent, TaskAudited, TaskCompleted, TaskCreated,
    TaskGateChecked, TaskImplemented, TaskStarted, TaskXChecked, fold_task_status,
};
use tanren_domain::methodology::task::{
    RequiredGuard, Task, TaskAbandonDisposition, TaskOrigin, TaskStatus,
};
use tanren_domain::{NonEmptyString, SpecId, TaskId};

fn seed_lifecycle(tid: TaskId, spec: SpecId) -> Vec<MethodologyEvent> {
    let task = Task {
        id: tid,
        spec_id: spec,
        title: NonEmptyString::try_new("t").expect("non-empty"),
        description: String::new(),
        acceptance_criteria: vec![],
        origin: TaskOrigin::ShapeSpec,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    vec![
        MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(task),
            origin: TaskOrigin::ShapeSpec,
            idempotency_key: None,
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
    ]
}

fn guard_event(tid: TaskId, spec: SpecId, guard: RequiredGuard) -> MethodologyEvent {
    match guard {
        RequiredGuard::GateChecked => MethodologyEvent::TaskGateChecked(TaskGateChecked {
            task_id: tid,
            spec_id: spec,
            idempotency_key: None,
        }),
        RequiredGuard::Audited => MethodologyEvent::TaskAudited(TaskAudited {
            task_id: tid,
            spec_id: spec,
            idempotency_key: None,
        }),
        RequiredGuard::Adherent => MethodologyEvent::TaskAdherent(TaskAdherent {
            task_id: tid,
            spec_id: spec,
            idempotency_key: None,
        }),
        RequiredGuard::Extra(name) => MethodologyEvent::TaskXChecked(TaskXChecked {
            task_id: tid,
            spec_id: spec,
            guard_name: NonEmptyString::try_new(name).expect("non-empty guard name"),
            idempotency_key: None,
        }),
    }
}

/// Any permutation of 0..3 expressed as a `Vec<usize>`.
fn guard_permutations() -> impl Strategy<Value = Vec<usize>> {
    Just(vec![0_usize, 1, 2]).prop_shuffle()
}

#[derive(Clone, Copy, Debug)]
enum LifecycleOp {
    Start,
    Implement,
    Gate,
    Audit,
    Adhere,
    Complete,
    Abandon,
}

fn lifecycle_streams() -> impl Strategy<Value = Vec<LifecycleOp>> {
    prop::collection::vec(0_u8..7, 1..40).prop_map(|raw| {
        raw.into_iter()
            .map(|code| match code {
                0 => LifecycleOp::Start,
                1 => LifecycleOp::Implement,
                2 => LifecycleOp::Gate,
                3 => LifecycleOp::Audit,
                4 => LifecycleOp::Adhere,
                5 => LifecycleOp::Complete,
                6 => LifecycleOp::Abandon,
                _ => unreachable!("bounded by strategy"),
            })
            .collect::<Vec<_>>()
    })
}

#[derive(Debug, Clone)]
struct TaskModel {
    status: Option<TaskStatus>,
    guards: tanren_domain::methodology::task::TaskGuardFlags,
}

impl TaskModel {
    fn new() -> Self {
        Self {
            status: Some(TaskStatus::Pending),
            guards: tanren_domain::methodology::task::TaskGuardFlags::default(),
        }
    }

    fn apply(&mut self, op: LifecycleOp, required: &[RequiredGuard]) {
        match op {
            LifecycleOp::Start => {
                if !matches!(
                    self.status,
                    Some(TaskStatus::Complete | TaskStatus::Abandoned { .. })
                ) {
                    self.status = Some(TaskStatus::InProgress);
                }
            }
            LifecycleOp::Implement => {
                if !matches!(
                    self.status,
                    Some(TaskStatus::Complete | TaskStatus::Abandoned { .. })
                ) {
                    self.status = Some(TaskStatus::Implemented {
                        guards: self.guards.clone(),
                    });
                }
            }
            LifecycleOp::Gate => self.apply_guard(&RequiredGuard::GateChecked),
            LifecycleOp::Audit => self.apply_guard(&RequiredGuard::Audited),
            LifecycleOp::Adhere => self.apply_guard(&RequiredGuard::Adherent),
            LifecycleOp::Complete => {
                if matches!(self.status, Some(TaskStatus::Implemented { .. }))
                    && self.guards.satisfies(required)
                {
                    self.status = Some(TaskStatus::Complete);
                }
            }
            LifecycleOp::Abandon => {
                if !matches!(self.status, Some(TaskStatus::Complete)) {
                    self.status = Some(TaskStatus::Abandoned {
                        disposition: TaskAbandonDisposition::Replacement,
                        replacements: Vec::new(),
                        explicit_user_discard_provenance: None,
                    });
                }
            }
        }
    }

    fn apply_guard(&mut self, guard: &RequiredGuard) {
        self.guards.set(guard, true);
        if matches!(self.status, Some(TaskStatus::Implemented { .. })) {
            self.status = Some(TaskStatus::Implemented {
                guards: self.guards.clone(),
            });
        }
    }
}

fn lifecycle_event(tid: TaskId, spec: SpecId, op: LifecycleOp) -> MethodologyEvent {
    match op {
        LifecycleOp::Start => MethodologyEvent::TaskStarted(TaskStarted {
            task_id: tid,
            spec_id: spec,
        }),
        LifecycleOp::Implement => MethodologyEvent::TaskImplemented(TaskImplemented {
            task_id: tid,
            spec_id: spec,
            evidence_refs: vec![],
        }),
        LifecycleOp::Gate => MethodologyEvent::TaskGateChecked(TaskGateChecked {
            task_id: tid,
            spec_id: spec,
            idempotency_key: None,
        }),
        LifecycleOp::Audit => MethodologyEvent::TaskAudited(TaskAudited {
            task_id: tid,
            spec_id: spec,
            idempotency_key: None,
        }),
        LifecycleOp::Adhere => MethodologyEvent::TaskAdherent(TaskAdherent {
            task_id: tid,
            spec_id: spec,
            idempotency_key: None,
        }),
        LifecycleOp::Complete => MethodologyEvent::TaskCompleted(TaskCompleted {
            task_id: tid,
            spec_id: spec,
        }),
        LifecycleOp::Abandon => MethodologyEvent::TaskAbandoned(TaskAbandoned {
            task_id: tid,
            spec_id: spec,
            reason: NonEmptyString::try_new("abandon").expect("non-empty"),
            disposition: TaskAbandonDisposition::Replacement,
            replacements: vec![],
            explicit_user_discard_provenance: None,
        }),
    }
}

proptest! {
    /// Guard-satisfaction events commute under permutation: every order
    /// of the three required guards followed by `TaskCompleted` yields
    /// `TaskStatus::Complete`.
    #[test]
    fn guard_events_commute_under_permutation(order in guard_permutations()) {
        let tid = TaskId::new();
        let spec = SpecId::new();
        let mut all = seed_lifecycle(tid, spec);
        let guards = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        for i in order {
            all.push(guard_event(tid, spec, guards[i].clone()));
        }
        all.push(MethodologyEvent::TaskCompleted(TaskCompleted {
            task_id: tid,
            spec_id: spec,
        }));
        let status = fold_task_status(tid, &guards, &all);
        prop_assert_eq!(status, Some(TaskStatus::Complete));
    }

    /// Complete is terminal: trailing TaskAbandoned events are ignored
    /// once the task has converged to Complete.
    #[test]
    fn complete_is_terminal_under_replay(n_extra in 0_usize..8) {
        let tid = TaskId::new();
        let spec = SpecId::new();
        let required = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        let mut all = seed_lifecycle(tid, spec);
        for g in required.iter().cloned() {
            all.push(guard_event(tid, spec, g));
        }
        all.push(MethodologyEvent::TaskCompleted(TaskCompleted {
            task_id: tid,
            spec_id: spec,
        }));
        for _ in 0..n_extra {
            all.push(MethodologyEvent::TaskAbandoned(TaskAbandoned {
                task_id: tid,
                spec_id: spec,
                reason: NonEmptyString::try_new("stray abandon").expect("non-empty"),
                disposition: TaskAbandonDisposition::Replacement,
                replacements: vec![],
                explicit_user_discard_provenance: None,
            }));
        }
        let status = fold_task_status(tid, &required, &all);
        prop_assert_eq!(status, Some(TaskStatus::Complete));
    }

    /// Without all required guards, a late `TaskCompleted` does NOT
    /// transition to Complete. The task stays Implemented with the
    /// observed subset of guards satisfied.
    #[test]
    fn completed_without_all_guards_stays_implemented(skip in 0_usize..3) {
        let tid = TaskId::new();
        let spec = SpecId::new();
        let required = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        let mut all = seed_lifecycle(tid, spec);
        for (i, g) in required.iter().cloned().enumerate() {
            if i != skip {
                all.push(guard_event(tid, spec, g));
            }
        }
        all.push(MethodologyEvent::TaskCompleted(TaskCompleted {
            task_id: tid,
            spec_id: spec,
        }));
        let status = fold_task_status(tid, &required, &all);
        let is_implemented = matches!(status, Some(TaskStatus::Implemented { .. }));
        prop_assert!(is_implemented);
    }

    /// Model-based lifecycle interleaving property:
    /// duplicates, out-of-order guards, and late events should fold to
    /// the same state as an independent state-machine model.
    #[test]
    fn lifecycle_interleavings_match_model(ops in lifecycle_streams()) {
        let tid = TaskId::new();
        let spec = SpecId::new();
        let required = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        let mut events = vec![MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(Task {
                id: tid,
                spec_id: spec,
                title: NonEmptyString::try_new("t").expect("non-empty"),
                description: String::new(),
                acceptance_criteria: vec![],
                origin: TaskOrigin::ShapeSpec,
                status: TaskStatus::Pending,
                depends_on: vec![],
                parent_task_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }),
            origin: TaskOrigin::ShapeSpec,
            idempotency_key: None,
        })];
        events.extend(ops.iter().copied().map(|op| lifecycle_event(tid, spec, op)));

        let folded = fold_task_status(tid, &required, &events);
        let mut model = TaskModel::new();
        for op in &ops {
            model.apply(*op, &required);
        }
        prop_assert_eq!(folded, model.status);
    }

    /// Prefix monotonicity property:
    /// once a prefix reaches `Complete`, no later suffix can regress it.
    #[test]
    fn complete_remains_terminal_under_arbitrary_suffixes(ops in lifecycle_streams()) {
        let tid = TaskId::new();
        let spec = SpecId::new();
        let required = [
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ];
        let mut events = vec![MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(Task {
                id: tid,
                spec_id: spec,
                title: NonEmptyString::try_new("t").expect("non-empty"),
                description: String::new(),
                acceptance_criteria: vec![],
                origin: TaskOrigin::ShapeSpec,
                status: TaskStatus::Pending,
                depends_on: vec![],
                parent_task_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }),
            origin: TaskOrigin::ShapeSpec,
            idempotency_key: None,
        })];
        events.extend(ops.iter().copied().map(|op| lifecycle_event(tid, spec, op)));

        let mut seen_complete = false;
        for end in 1..=events.len() {
            let status = fold_task_status(tid, &required, events[..end].iter());
            if seen_complete {
                prop_assert_eq!(status.clone(), Some(TaskStatus::Complete));
            }
            if status == Some(TaskStatus::Complete) {
                seen_complete = true;
            }
        }
    }
}
