//! Read-path policy scope checks for orchestrator dispatch queries.

use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigEnv, CreateDispatch, DispatchMode, NonEmptyString, OrgId,
    Phase, TimeoutSecs, UserId,
};
use tanren_orchestrator::Orchestrator;
use tanren_policy::PolicyEngine;
use tanren_store::{ReplayGuard, Store};
use uuid::Uuid;

fn sample_command(actor: ActorContext) -> CreateDispatch {
    CreateDispatch {
        actor,
        project: NonEmptyString::try_new("test-project".to_owned()).expect("project"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("workflow"),
        mode: DispatchMode::Manual,
        timeout: TimeoutSecs::try_new(60).expect("timeout"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("profile"),
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigEnv::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
    }
}

fn sample_actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

fn sample_replay_guard() -> ReplayGuard {
    ReplayGuard {
        issuer: "tanren-tests".to_owned(),
        audience: "tanren-cli".to_owned(),
        jti: Uuid::now_v7().to_string(),
        iat_unix: 10,
        exp_unix: 20,
    }
}

async fn setup() -> Orchestrator<Store> {
    let store = Store::open_and_migrate("sqlite::memory:")
        .await
        .expect("store");
    Orchestrator::new(store, PolicyEngine::new())
}

#[tokio::test]
async fn get_dispatch_for_actor_denies_scope_mismatch() {
    let orch = setup().await;
    let created = orch
        .create_dispatch(sample_command(sample_actor()), sample_replay_guard())
        .await
        .expect("create");

    let view = orch
        .get_dispatch_for_actor(
            &created.dispatch_id,
            &ActorContext::new(OrgId::new(), UserId::new()),
        )
        .await
        .expect("get should not error");
    assert!(
        view.is_none(),
        "scope mismatch should be hidden as not found-equivalent"
    );
}

#[tokio::test]
async fn list_dispatches_for_actor_hides_unauthorized_dispatches() {
    let orch = setup().await;
    let _ = orch
        .create_dispatch(sample_command(sample_actor()), sample_replay_guard())
        .await
        .expect("create");

    let page = orch
        .list_dispatches_for_actor(
            tanren_store::DispatchFilter::new(),
            &ActorContext::new(OrgId::new(), UserId::new()),
        )
        .await
        .expect("list");
    assert!(
        page.dispatches.is_empty(),
        "unauthorized actor should not see dispatch rows"
    );
}

#[tokio::test]
async fn list_dispatches_for_actor_allows_cross_user_with_matching_scope() {
    let orch = setup().await;
    let org = OrgId::new();
    let project = tanren_domain::ProjectId::new();
    let creator = ActorContext {
        org_id: org,
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: Some(project),
    };
    let reader = ActorContext {
        org_id: org,
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: Some(project),
    };

    let _ = orch
        .create_dispatch(sample_command(creator), sample_replay_guard())
        .await
        .expect("create");

    let page = orch
        .list_dispatches_for_actor(tanren_store::DispatchFilter::new(), &reader)
        .await
        .expect("list");
    assert_eq!(page.dispatches.len(), 1);
}
