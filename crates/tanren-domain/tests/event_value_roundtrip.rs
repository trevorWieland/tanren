//! `SeaORM` `JsonBinary` round-trip regression tests for every
//! `DomainEvent` variant.
//!
//! These tests mirror the assertions inside the `snapshot_event!` macro
//! in `tests/snapshot_events.rs` but put them under named, category-
//! scoped `#[test]` functions so a failure in the `SeaORM` contract
//! surfaces with a diagnostic category (`dispatch`, `step`, `lease`, or
//! `policy`) rather than being buried inside a generated per-variant
//! snapshot test. This file holds the regression intact even if the
//! snapshot file is ever restructured.

use chrono::{TimeZone, Utc};
use tanren_domain::actor::ActorContext;
use tanren_domain::commands::LeaseCapabilities;
use tanren_domain::errors::ErrorClass;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::graph::GraphRevision;
use tanren_domain::ids::{DispatchId, EventId, LeaseId, OrgId, StepId, UserId};
use tanren_domain::payloads::{
    ConfigKeys, DispatchSnapshot, ExecuteResult, StepResult, TokenUsage,
};
use tanren_domain::policy::{
    PolicyDecisionKind, PolicyDecisionRecord, PolicyOutcome, PolicyResourceRef, PolicyScope,
};
use tanren_domain::status::{AuthMode, Cli, DispatchMode, Lane, Outcome, Phase, StepType};
use tanren_domain::validated::{FiniteF64, NonEmptyString, TimeoutSecs};

// -- Fixed-value helpers -------------------------------------------------

fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0)
        .single()
        .expect("valid timestamp")
}

fn fixed_uuid() -> uuid::Uuid {
    uuid::Uuid::parse_str("01966a00-0000-7000-8000-000000000001").expect("valid uuid")
}

fn did() -> DispatchId {
    DispatchId::from_uuid(fixed_uuid())
}
fn sid() -> StepId {
    StepId::from_uuid(fixed_uuid())
}
fn lid() -> LeaseId {
    LeaseId::from_uuid(fixed_uuid())
}
fn uid() -> UserId {
    UserId::from_uuid(fixed_uuid())
}
fn oid() -> OrgId {
    OrgId::from_uuid(fixed_uuid())
}
fn eid() -> EventId {
    EventId::from_uuid(fixed_uuid())
}

fn actor() -> ActorContext {
    ActorContext::new(oid(), uid())
}

fn nes(s: &str) -> NonEmptyString {
    NonEmptyString::try_new(s).expect("valid non-empty string")
}

fn finite(v: f64) -> FiniteF64 {
    FiniteF64::try_new(v).expect("finite literal")
}

fn sample_snapshot() -> Box<DispatchSnapshot> {
    Box::new(DispatchSnapshot {
        project: nes("acme"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: nes("main"),
        spec_folder: nes("specs/"),
        workflow_id: nes("wf-acme-42-1718452800"),
        timeout: TimeoutSecs::try_new(3600).expect("valid timeout"),
        environment_profile: nes("default"),
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigKeys::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
        created_at: ts(),
    })
}

fn make_envelope(event: DomainEvent) -> EventEnvelope {
    EventEnvelope::new(eid(), ts(), event)
}

/// Assert that every event in `variants` round-trips through the
/// `serde_json::Value` path — the exact API `SeaORM` uses for
/// `JsonBinary` columns.
fn assert_value_path_roundtrips(variants: Vec<DomainEvent>) {
    for event in variants {
        let envelope = make_envelope(event);
        let value = serde_json::to_value(&envelope).expect("to_value");
        let back: EventEnvelope = serde_json::from_value(value).expect("from_value");
        assert_eq!(envelope, back, "round-trip failed for variant");
    }
}

// -- Regression tests ----------------------------------------------------

/// Every dispatch-lifecycle variant survives the `SeaORM` `JsonBinary`
/// round-trip (`to_value` → `from_value`).
#[test]
fn every_dispatch_variant_survives_value_path() {
    assert_value_path_roundtrips(vec![
        DomainEvent::DispatchCreated {
            dispatch_id: did(),
            dispatch: sample_snapshot(),
            mode: DispatchMode::Auto,
            lane: Lane::Impl,
            actor: actor(),
            graph_revision: GraphRevision::INITIAL,
        },
        DomainEvent::DispatchStarted { dispatch_id: did() },
        DomainEvent::DispatchCompleted {
            dispatch_id: did(),
            outcome: Outcome::Success,
            total_duration_secs: finite(120.5),
        },
        DomainEvent::DispatchFailed {
            dispatch_id: did(),
            outcome: Outcome::Error,
            failed_step_id: Some(sid()),
            failed_step_type: Some(StepType::Execute),
            error: "harness exited with code 1".into(),
        },
        DomainEvent::DispatchCancelled {
            dispatch_id: did(),
            actor: actor(),
            reason: Some("user requested".into()),
        },
    ]);
}

/// Every step-lifecycle variant survives the `SeaORM` `JsonBinary`
/// round-trip.
#[test]
fn every_step_variant_survives_value_path() {
    assert_value_path_roundtrips(vec![
        DomainEvent::StepEnqueued {
            dispatch_id: did(),
            step_id: sid(),
            step_type: StepType::Provision,
            step_sequence: 1,
            lane: None,
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
        },
        DomainEvent::StepDequeued {
            dispatch_id: did(),
            step_id: sid(),
            worker_id: "worker-1".into(),
        },
        DomainEvent::StepStarted {
            dispatch_id: did(),
            step_id: sid(),
            worker_id: "worker-1".into(),
            step_type: StepType::Execute,
        },
        DomainEvent::StepCompleted {
            dispatch_id: did(),
            step_id: sid(),
            step_type: StepType::Execute,
            duration_secs: finite(45.2),
            result_payload: Box::new(StepResult::Execute(Box::new(ExecuteResult {
                outcome: Outcome::Success,
                signal: None,
                exit_code: Some(0),
                duration_secs: finite(45.2),
                gate_output: None,
                tail_output: None,
                stderr_tail: None,
                pushed: false,
                plan_hash: None,
                unchecked_tasks: 0,
                spec_modified: false,
                findings: vec![],
                token_usage: Some(TokenUsage::default()),
            }))),
        },
        DomainEvent::StepFailed {
            dispatch_id: did(),
            step_id: sid(),
            step_type: StepType::Execute,
            error: "timeout".into(),
            error_class: ErrorClass::Transient,
            retry_count: 1,
            duration_secs: finite(3600.0),
        },
        DomainEvent::StepCancelled {
            dispatch_id: did(),
            step_id: sid(),
            step_type: StepType::Provision,
            caused_by: None,
            reason: None,
        },
    ]);
}

/// Every lease-lifecycle variant survives the `SeaORM` `JsonBinary`
/// round-trip.
#[test]
fn every_lease_variant_survives_value_path() {
    assert_value_path_roundtrips(vec![
        DomainEvent::LeaseRequested {
            lease_id: lid(),
            dispatch_id: did(),
            step_id: sid(),
            capabilities: Box::new(LeaseCapabilities {
                runtime_type: nes("docker"),
                resource_limits: None,
                network_policy: None,
                mount_requirements: vec![],
            }),
        },
        DomainEvent::LeaseProvisioned {
            lease_id: lid(),
            dispatch_id: did(),
            runtime_type: "docker".into(),
        },
        DomainEvent::LeaseReady {
            lease_id: lid(),
            dispatch_id: did(),
        },
        DomainEvent::LeaseRunning {
            lease_id: lid(),
            dispatch_id: did(),
            step_id: sid(),
        },
        DomainEvent::LeaseIdle {
            lease_id: lid(),
            dispatch_id: did(),
        },
        DomainEvent::LeaseDraining {
            lease_id: lid(),
            dispatch_id: did(),
            caused_by: Some(actor()),
            reason: Some("user release".into()),
        },
        DomainEvent::LeaseReleased {
            lease_id: lid(),
            dispatch_id: did(),
            duration_secs: finite(300.0),
            caused_by: None,
        },
        DomainEvent::LeaseFailed {
            lease_id: lid(),
            dispatch_id: did(),
            error: "provisioning timeout".into(),
        },
    ]);
}

/// The policy-decision variant survives the `SeaORM` `JsonBinary`
/// round-trip.
#[test]
fn policy_decision_variant_survives_value_path() {
    assert_value_path_roundtrips(vec![DomainEvent::PolicyDecision {
        dispatch_id: did(),
        decision: Box::new(PolicyDecisionRecord {
            kind: PolicyDecisionKind::Budget,
            resource: PolicyResourceRef::Dispatch { dispatch_id: did() },
            scope: PolicyScope::new(actor()),
            outcome: PolicyOutcome::Allowed,
            reason: Some("within monthly budget".into()),
        }),
    }]);
}
