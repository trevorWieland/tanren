//! `SQLite` policy decision event append tests.

use tanren_domain::{
    ActorContext, DispatchId, DomainEvent, EntityRef, EventEnvelope, EventId, OrgId,
    PolicyDecisionKind, PolicyDecisionRecord, PolicyOutcome, PolicyReasonCode, PolicyResourceRef,
    PolicyScope, UserId,
};
use tanren_store::{EventFilter, EventStore, ReplayGuard, Store, StoreError};

async fn fresh_store() -> Store {
    let store = Store::new("sqlite::memory:").await.expect("connect");
    store.run_migrations().await.expect("migrate");
    store
}

fn actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

#[tokio::test]
async fn append_policy_decision_event_persists_and_is_queryable() {
    let store = fresh_store().await;
    let dispatch_id = DispatchId::new();
    let decision = PolicyDecisionRecord {
        kind: PolicyDecisionKind::Authz,
        resource: PolicyResourceRef::Dispatch { dispatch_id },
        scope: PolicyScope::new(actor()),
        outcome: PolicyOutcome::Denied,
        reason_code: Some(PolicyReasonCode::CancelOrgMismatch),
        reason: Some("org mismatch".to_owned()),
    };
    let event = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        now(),
        DomainEvent::PolicyDecision {
            dispatch_id,
            decision: Box::new(decision),
        },
    );

    store
        .append_policy_decision_event(&event)
        .await
        .expect("append policy decision");
    let queried = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(dispatch_id)),
            include_total_count: true,
            ..EventFilter::new()
        })
        .await
        .expect("query events");
    assert_eq!(queried.total_count, Some(1));
    assert!(matches!(
        queried.events[0].payload,
        DomainEvent::PolicyDecision { .. }
    ));
}

#[tokio::test]
async fn append_policy_decision_event_rejects_non_policy_payload() {
    let store = fresh_store().await;
    let dispatch_id = DispatchId::new();
    let event = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        now(),
        DomainEvent::DispatchStarted { dispatch_id },
    );
    let err = store
        .append_policy_decision_event(&event)
        .await
        .expect_err("append should fail");
    assert!(matches!(err, StoreError::Conversion { .. }));
}

fn policy_decision_event(dispatch_id: DispatchId) -> EventEnvelope {
    let decision = PolicyDecisionRecord {
        kind: PolicyDecisionKind::Authz,
        resource: PolicyResourceRef::Dispatch { dispatch_id },
        scope: PolicyScope::new(actor()),
        outcome: PolicyOutcome::Denied,
        reason_code: Some(PolicyReasonCode::CancelOrgMismatch),
        reason: Some("org mismatch".to_owned()),
    };
    EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        now(),
        DomainEvent::PolicyDecision {
            dispatch_id,
            decision: Box::new(decision),
        },
    )
}

fn sample_replay_guard() -> ReplayGuard {
    ReplayGuard {
        issuer: "tanren-tests".to_owned(),
        audience: "tanren-cli".to_owned(),
        jti: uuid::Uuid::now_v7().to_string(),
        iat_unix: 10,
        exp_unix: 20,
    }
}

#[tokio::test]
async fn record_policy_decision_with_replay_consumes_replay_and_persists_event() {
    let store = fresh_store().await;
    let dispatch_id = DispatchId::new();
    let event = policy_decision_event(dispatch_id);
    let replay = sample_replay_guard();

    store
        .record_policy_decision_with_replay(&event, replay)
        .await
        .expect("first record should succeed");

    let queried = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(dispatch_id)),
            include_total_count: true,
            ..EventFilter::new()
        })
        .await
        .expect("query events");
    assert_eq!(queried.total_count, Some(1));
    assert!(matches!(
        queried.events[0].payload,
        DomainEvent::PolicyDecision { .. }
    ));
}

#[tokio::test]
async fn record_policy_decision_with_replay_rejects_second_use_of_jti() {
    let store = fresh_store().await;
    let dispatch_id = DispatchId::new();
    let replay = sample_replay_guard();

    store
        .record_policy_decision_with_replay(&policy_decision_event(dispatch_id), replay.clone())
        .await
        .expect("first record succeeds");

    let second_id = DispatchId::new();
    let err = store
        .record_policy_decision_with_replay(&policy_decision_event(second_id), replay)
        .await
        .expect_err("second record must be replay-rejected");
    assert!(
        matches!(err, StoreError::ReplayRejected),
        "expected ReplayRejected, got {err:?}"
    );

    let queried = store
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(second_id)),
            include_total_count: true,
            ..EventFilter::new()
        })
        .await
        .expect("query events");
    assert_eq!(
        queried.total_count,
        Some(0),
        "replay-rejected attempt must not persist an event"
    );
}

#[tokio::test]
async fn record_policy_decision_with_replay_rejects_non_policy_payload() {
    let store = fresh_store().await;
    let dispatch_id = DispatchId::new();
    let event = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        now(),
        DomainEvent::DispatchStarted { dispatch_id },
    );
    let err = store
        .record_policy_decision_with_replay(&event, sample_replay_guard())
        .await
        .expect_err("non-policy payload must be rejected");
    assert!(matches!(err, StoreError::Conversion { .. }));
}
