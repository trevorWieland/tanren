//! Serde round-trip snapshot tests for all domain event variants.

use chrono::{TimeZone, Utc};
use tanren_domain::actor::ActorContext;
use tanren_domain::commands::LeaseCapabilities;
use tanren_domain::errors::ErrorClass;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::ids::{DispatchId, EventId, LeaseId, OrgId, StepId, UserId};
use tanren_domain::payloads::{
    ConfigKeys, DispatchSnapshot, ExecuteResult, StepResult, TokenUsage,
};
use tanren_domain::policy::{
    PolicyDecisionKind, PolicyDecisionRecord, PolicyOutcome, PolicyResourceRef, PolicyScope,
};
use tanren_domain::status::{AuthMode, Cli, DispatchMode, Lane, Outcome, Phase, StepType};
use tanren_domain::validated::{NonEmptyString, TimeoutSecs};

/// Fixed timestamp for deterministic snapshots.
fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0)
        .single()
        .expect("valid timestamp")
}

/// Fixed UUID for deterministic snapshots.
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

macro_rules! snapshot_event {
    ($name:ident, $event:expr) => {
        #[test]
        fn $name() {
            let envelope = make_envelope($event);
            let json = serde_json::to_string_pretty(&envelope).expect("serialize");

            insta::assert_snapshot!(json);

            // Verify round-trip.
            let back: EventEnvelope = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(envelope, back);
        }
    };
}

snapshot_event!(
    dispatch_created,
    DomainEvent::DispatchCreated {
        dispatch_id: did(),
        dispatch: sample_snapshot(),
        mode: DispatchMode::Auto,
        lane: Lane::Impl,
        actor: actor(),
        graph_revision: 1,
        timestamp: ts(),
    }
);

snapshot_event!(
    dispatch_started,
    DomainEvent::DispatchStarted {
        dispatch_id: did(),
        timestamp: ts(),
    }
);

snapshot_event!(
    dispatch_completed,
    DomainEvent::DispatchCompleted {
        dispatch_id: did(),
        outcome: Outcome::Success,
        total_duration_secs: 120.5,
        timestamp: ts(),
    }
);

snapshot_event!(
    dispatch_failed,
    DomainEvent::DispatchFailed {
        dispatch_id: did(),
        outcome: Outcome::Error,
        failed_step_id: Some(sid()),
        failed_step_type: Some(StepType::Execute),
        error: "harness exited with code 1".into(),
        timestamp: ts(),
    }
);

snapshot_event!(
    dispatch_cancelled,
    DomainEvent::DispatchCancelled {
        dispatch_id: did(),
        actor: actor(),
        reason: Some("user requested".into()),
        timestamp: ts(),
    }
);

snapshot_event!(
    step_enqueued,
    DomainEvent::StepEnqueued {
        dispatch_id: did(),
        step_id: sid(),
        step_type: StepType::Provision,
        step_sequence: 1,
        lane: None,
        depends_on: vec![],
        graph_revision: 1,
        timestamp: ts(),
    }
);

snapshot_event!(
    step_dequeued,
    DomainEvent::StepDequeued {
        dispatch_id: did(),
        step_id: sid(),
        worker_id: "worker-1".into(),
        timestamp: ts(),
    }
);

snapshot_event!(
    step_started,
    DomainEvent::StepStarted {
        dispatch_id: did(),
        step_id: sid(),
        worker_id: "worker-1".into(),
        step_type: StepType::Execute,
        timestamp: ts(),
    }
);

snapshot_event!(
    step_completed,
    DomainEvent::StepCompleted {
        dispatch_id: did(),
        step_id: sid(),
        step_type: StepType::Execute,
        duration_secs: 45.2,
        result_payload: Box::new(StepResult::Execute(Box::new(ExecuteResult {
            outcome: Outcome::Success,
            signal: None,
            exit_code: Some(0),
            duration_secs: 45.2,
            gate_output: None,
            tail_output: Some("All tasks completed".into()),
            stderr_tail: None,
            pushed: true,
            plan_hash: Some("abc123".into()),
            unchecked_tasks: 0,
            spec_modified: false,
            findings: vec![],
            token_usage: Some(TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: 200,
                cache_write_tokens: 100,
            }),
        }))),
        timestamp: ts(),
    }
);

snapshot_event!(
    step_failed,
    DomainEvent::StepFailed {
        dispatch_id: did(),
        step_id: sid(),
        step_type: StepType::Execute,
        error: "timeout after 3600s".into(),
        error_class: ErrorClass::Transient,
        retry_count: 1,
        duration_secs: 3600.0,
        timestamp: ts(),
    }
);

snapshot_event!(
    step_cancelled,
    DomainEvent::StepCancelled {
        dispatch_id: did(),
        step_id: sid(),
        step_type: StepType::Provision,
        caused_by: Some(actor()),
        reason: Some("dispatch cancelled".into()),
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_requested,
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
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_provisioned,
    DomainEvent::LeaseProvisioned {
        lease_id: lid(),
        dispatch_id: did(),
        runtime_type: "docker".into(),
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_ready,
    DomainEvent::LeaseReady {
        lease_id: lid(),
        dispatch_id: did(),
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_running,
    DomainEvent::LeaseRunning {
        lease_id: lid(),
        dispatch_id: did(),
        step_id: sid(),
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_idle,
    DomainEvent::LeaseIdle {
        lease_id: lid(),
        dispatch_id: did(),
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_draining_user,
    DomainEvent::LeaseDraining {
        lease_id: lid(),
        dispatch_id: did(),
        caused_by: Some(actor()),
        reason: Some("user release".into()),
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_draining_auto,
    DomainEvent::LeaseDraining {
        lease_id: lid(),
        dispatch_id: did(),
        caused_by: None,
        reason: Some("idle timeout".into()),
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_released,
    DomainEvent::LeaseReleased {
        lease_id: lid(),
        dispatch_id: did(),
        duration_secs: 300.0,
        caused_by: None,
        timestamp: ts(),
    }
);

snapshot_event!(
    lease_failed,
    DomainEvent::LeaseFailed {
        lease_id: lid(),
        dispatch_id: did(),
        error: "provisioning timeout".into(),
        timestamp: ts(),
    }
);

snapshot_event!(
    policy_decision,
    DomainEvent::PolicyDecision {
        dispatch_id: did(),
        decision: Box::new(PolicyDecisionRecord {
            kind: PolicyDecisionKind::Budget,
            resource: PolicyResourceRef::Dispatch { dispatch_id: did() },
            scope: PolicyScope::new(actor()),
            outcome: PolicyOutcome::Allowed,
            reason: Some("within monthly budget".into()),
        }),
        timestamp: ts(),
    }
);

// -- dispatch_id accessor ------------------------------------------------

#[test]
fn dispatch_id_accessor_returns_correlated_id_for_every_variant() {
    let dispatch_event = DomainEvent::DispatchStarted {
        dispatch_id: did(),
        timestamp: ts(),
    };
    assert_eq!(dispatch_event.dispatch_id(), did());

    let step_event = DomainEvent::StepCancelled {
        dispatch_id: did(),
        step_id: sid(),
        step_type: StepType::Provision,
        caused_by: None,
        reason: None,
        timestamp: ts(),
    };
    assert_eq!(step_event.dispatch_id(), did());

    let lease_event = DomainEvent::LeaseProvisioned {
        lease_id: lid(),
        dispatch_id: did(),
        runtime_type: "docker".into(),
        timestamp: ts(),
    };
    assert_eq!(lease_event.dispatch_id(), did());
}
