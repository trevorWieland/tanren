//! Inline unit-style tests for `methodology::events`, moved out of the
//! module so `events.rs` stays under the 500-line file budget after
//! the Lane 0.5 audit-remediation additions.

use chrono::Utc;
use tanren_domain::entity::EntityRef;
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskCompleted, TaskCreated, TaskGuardSatisfied, TaskImplemented, TaskStarted,
    fold_task_status,
};
use tanren_domain::methodology::task::{
    RequiredGuard, Task, TaskGuardFlags, TaskOrigin, TaskStatus,
};
use tanren_domain::validated::NonEmptyString;
use tanren_domain::{SpecId, TaskId};

fn ne(s: &str) -> NonEmptyString {
    NonEmptyString::try_new(s).expect("non-empty")
}

fn seed_task(spec: SpecId) -> Task {
    Task {
        id: TaskId::new(),
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
fn canonical_guard_name_mapping() {
    let spec = SpecId::new();
    let tid = TaskId::new();
    let cases = [
        (RequiredGuard::GateChecked, "TaskGateChecked"),
        (RequiredGuard::Audited, "TaskAudited"),
        (RequiredGuard::Adherent, "TaskAdherent"),
        (
            RequiredGuard::Extra("throughput_checked".into()),
            "TaskXChecked",
        ),
    ];
    for (guard, expected) in cases {
        let ev = TaskGuardSatisfied {
            task_id: tid,
            spec_id: spec,
            guard,
            idempotency_key: None,
        };
        assert_eq!(ev.canonical_event_name(), expected);
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
            idempotency_key: None,
        }),
        MethodologyEvent::TaskGuardSatisfied(TaskGuardSatisfied {
            task_id: tid,
            spec_id: spec,
            guard: RequiredGuard::Audited,
            idempotency_key: None,
        }),
        MethodologyEvent::TaskGuardSatisfied(TaskGuardSatisfied {
            task_id: tid,
            spec_id: spec,
            guard: RequiredGuard::Adherent,
            idempotency_key: None,
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
            idempotency_key: None,
        }),
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
