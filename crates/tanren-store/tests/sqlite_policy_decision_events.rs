//! `SQLite` policy decision event append tests.

use tanren_domain::{
    ActorContext, DispatchId, DomainEvent, EntityRef, EventEnvelope, EventId, OrgId,
    PolicyDecisionKind, PolicyDecisionRecord, PolicyOutcome, PolicyReasonCode, PolicyResourceRef,
    PolicyScope, UserId,
};
use tanren_store::{EventFilter, EventStore, Store, StoreError};

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
