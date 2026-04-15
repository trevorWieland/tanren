use std::collections::HashMap;

use tanren_domain::{
    ActorContext, AuthMode, Cli, ConfigEnv, CreateDispatch, DispatchMode, NonEmptyString, OrgId,
    Phase, TimeoutSecs, UserId,
};
use tanren_store::ReplayGuard;
use uuid::Uuid;

pub(crate) fn sample_actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

pub(crate) fn sample_command(actor: ActorContext) -> CreateDispatch {
    CreateDispatch {
        actor,
        project: NonEmptyString::try_new("test-project".to_owned()).expect("non-empty"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("non-empty"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("non-empty"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("non-empty"),
        mode: DispatchMode::Manual,
        timeout: TimeoutSecs::try_new(60).expect("positive"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("non-empty"),
        gate_cmd: Some("cargo test".to_owned()),
        context: Some("context".to_owned()),
        model: Some("claude-4".to_owned()),
        project_env: ConfigEnv::from(HashMap::from([(
            "API_URL".to_owned(),
            "https://example.com".to_owned(),
        )])),
        required_secrets: vec!["OPENAI_API_KEY".to_owned()],
        preserve_on_failure: true,
    }
}

pub(crate) fn sample_replay_guard() -> ReplayGuard {
    ReplayGuard {
        issuer: "tanren-tests".to_owned(),
        audience: "tanren-cli".to_owned(),
        jti: Uuid::now_v7().to_string(),
        iat_unix: 10,
        exp_unix: 20,
    }
}
