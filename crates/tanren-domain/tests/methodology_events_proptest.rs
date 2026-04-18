//! Property tests for methodology event monotonicity and guard
//! composition.
//!
//! Non-negotiable #1: task state is monotonic. `Complete` is terminal.
//! Guards satisfy in any order and converge to the same final state.

use chrono::Utc;
use proptest::prelude::*;

use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAbandoned, TaskCompleted, TaskCreated, TaskGuardSatisfied,
    TaskImplemented, TaskStarted, fold_task_status,
};
use tanren_domain::methodology::task::{RequiredGuard, Task, TaskOrigin, TaskStatus};
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
    MethodologyEvent::TaskGuardSatisfied(TaskGuardSatisfied {
        task_id: tid,
        spec_id: spec,
        guard,
        idempotency_key: None,
    })
}

/// Any permutation of 0..3 expressed as a `Vec<usize>`.
fn guard_permutations() -> impl Strategy<Value = Vec<usize>> {
    Just(vec![0_usize, 1, 2]).prop_shuffle()
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
                replacements: vec![],
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
}
