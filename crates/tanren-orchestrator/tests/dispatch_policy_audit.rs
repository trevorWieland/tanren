//! Integration tests for policy decision audit event emission.

use tanren_domain::{
    ActorContext, AuthMode, ConfigEnv, CreateDispatch, DispatchMode, DomainError, EntityRef,
    NonEmptyString, OrgId, Phase, PolicyReasonCode, TimeoutSecs, UserId,
};
use tanren_orchestrator::{Orchestrator, OrchestratorError};
use tanren_policy::PolicyEngine;
use tanren_store::{EventFilter, EventStore, Store};

fn sample_actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

fn sample_command(actor: ActorContext) -> CreateDispatch {
    CreateDispatch {
        actor,
        project: NonEmptyString::try_new("test-project".to_owned()).expect("non-empty"),
        phase: Phase::DoTask,
        cli: tanren_domain::Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("non-empty"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("non-empty"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("non-empty"),
        mode: DispatchMode::Manual,
        timeout: TimeoutSecs::try_new(60).expect("positive"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("non-empty"),
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigEnv::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
    }
}

async fn setup() -> Orchestrator<Store> {
    let store = Store::open_and_migrate("sqlite::memory:")
        .await
        .expect("store");
    Orchestrator::new(store, PolicyEngine::new())
}

#[tokio::test]
async fn denied_create_emits_policy_decision_event() {
    let orch = setup().await;
    let mut cmd = sample_command(sample_actor());
    cmd.mode = DispatchMode::Auto;
    cmd.preserve_on_failure = true;

    let err = orch
        .create_dispatch(cmd)
        .await
        .expect_err("create should be denied");
    assert!(matches!(err, OrchestratorError::PolicyDenied { .. }));

    let OrchestratorError::PolicyDenied { decision } = err else {
        return;
    };
    let tanren_domain::PolicyResourceRef::Dispatch { dispatch_id } = decision.resource else {
        return;
    };

    let events = orch
        .store()
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(dispatch_id)),
            ..EventFilter::new()
        })
        .await
        .expect("events");
    assert_eq!(events.events.len(), 1);
    assert!(matches!(
        events.events[0].payload,
        tanren_domain::DomainEvent::PolicyDecision { .. }
    ));
}

#[tokio::test]
async fn denied_cancel_emits_policy_decision_event_and_returns_not_found() {
    let orch = setup().await;
    let created = orch
        .create_dispatch(sample_command(sample_actor()))
        .await
        .expect("create");

    let err = orch
        .cancel_dispatch(tanren_domain::CancelDispatch {
            actor: ActorContext::new(OrgId::new(), UserId::new()),
            dispatch_id: created.dispatch_id,
            reason: Some("forbidden".to_owned()),
        })
        .await
        .expect_err("cancel should fail");
    assert!(matches!(
        err,
        OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(_),
        })
    ));

    let events = orch
        .store()
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(created.dispatch_id)),
            ..EventFilter::new()
        })
        .await
        .expect("events");
    assert!(events.events.iter().any(|event| {
        matches!(
            &event.payload,
            tanren_domain::DomainEvent::PolicyDecision { decision, .. }
                if decision.reason_code == Some(PolicyReasonCode::CancelOrgMismatch)
        )
    }));
    assert!(
        events.events.iter().all(|event| !matches!(
            event.payload,
            tanren_domain::DomainEvent::DispatchCancelled { .. }
        )),
        "unauthorized cancel attempts must not append DispatchCancelled events"
    );
}

#[tokio::test]
async fn missing_cancel_emits_policy_decision_event_and_returns_not_found() {
    let orch = setup().await;
    let missing_id = tanren_domain::DispatchId::new();

    let err = orch
        .cancel_dispatch(tanren_domain::CancelDispatch {
            actor: sample_actor(),
            dispatch_id: missing_id,
            reason: Some("missing".to_owned()),
        })
        .await
        .expect_err("cancel should fail");
    assert!(matches!(
        err,
        OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(_),
        })
    ));

    let events = orch
        .store()
        .query_events(&EventFilter {
            entity_ref: Some(EntityRef::Dispatch(missing_id)),
            ..EventFilter::new()
        })
        .await
        .expect("events");
    assert!(events.events.iter().any(|event| {
        matches!(
            &event.payload,
            tanren_domain::DomainEvent::PolicyDecision { decision, .. }
                if decision.reason_code == Some(PolicyReasonCode::CancelDispatchNotFound)
        )
    }));
    assert_eq!(
        events.events.len(),
        1,
        "missing cancel should only append a policy decision audit event"
    );
    assert!(
        events.events.iter().all(|event| !matches!(
            event.payload,
            tanren_domain::DomainEvent::DispatchCancelled { .. }
        )),
        "missing cancel attempts must not append DispatchCancelled events"
    );
}
