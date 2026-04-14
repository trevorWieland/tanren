use tanren_domain::{
    ActorContext, ApiKeyId, AuthMode, CancelDispatch, Cli, ConfigEnv, CreateDispatch, DispatchId,
    DispatchMode, NonEmptyString, OrgId, Phase, PolicyOutcome, PolicyReasonCode, ProjectId, TeamId,
    TimeoutSecs, UserId,
};
use tanren_policy::{PolicyEngine, PolicyLimits};

fn sample_command() -> CreateDispatch {
    CreateDispatch {
        actor: ActorContext::new(OrgId::new(), UserId::new()),
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
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigEnv::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
    }
}

#[test]
fn strict_policy_allows_valid_request() {
    let engine = PolicyEngine::new();
    let decision = engine
        .check_dispatch_allowed(&sample_command(), DispatchId::new())
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Allowed);
    assert_eq!(decision.reason_code, None);
    assert_eq!(decision.reason.as_deref(), Some("allowed"));
}

#[test]
fn timeout_out_of_range_is_denied_with_code() {
    let mut cmd = sample_command();
    cmd.timeout = TimeoutSecs::try_new(29).expect("positive");
    let decision = PolicyEngine::new()
        .check_dispatch_allowed(&cmd, DispatchId::new())
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::TimeoutOutOfRange)
    );
}

#[test]
fn project_env_value_too_long_is_denied() {
    let limits = PolicyLimits {
        max_project_env_value_len: 3,
        ..PolicyLimits::default()
    };
    let mut cmd = sample_command();
    cmd.project_env = ConfigEnv::from(std::collections::HashMap::from([(
        "VALID_KEY".to_owned(),
        "value".to_owned(),
    )]));
    let decision = PolicyEngine::with_limits(limits)
        .check_dispatch_allowed(&cmd, DispatchId::new())
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::ProjectEnvValueTooLong)
    );
}

#[test]
fn secret_name_too_long_is_denied() {
    let limits = PolicyLimits {
        max_secret_name_len: 4,
        ..PolicyLimits::default()
    };
    let mut cmd = sample_command();
    cmd.required_secrets = vec!["TOO_LONG".to_owned()];
    let decision = PolicyEngine::with_limits(limits)
        .check_dispatch_allowed(&cmd, DispatchId::new())
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::SecretNameTooLong)
    );
}

#[test]
fn preserve_on_failure_requires_manual_mode() {
    let mut cmd = sample_command();
    cmd.mode = DispatchMode::Auto;
    cmd.preserve_on_failure = true;
    let decision = PolicyEngine::new()
        .check_dispatch_allowed(&cmd, DispatchId::new())
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::PreserveOnFailureRequiresManualMode)
    );
}

#[test]
fn disallowed_phase_cli_mode_is_denied() {
    let mut cmd = sample_command();
    cmd.phase = Phase::AuditSpec;
    cmd.cli = Cli::Bash;
    let decision = PolicyEngine::new()
        .check_dispatch_allowed(&cmd, DispatchId::new())
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::PhaseCliModeDisallowed)
    );
}

#[test]
fn cancel_allowed_when_scope_matches_dispatch() {
    let engine = PolicyEngine::new();
    let dispatch_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: Some(TeamId::new()),
        api_key_id: Some(ApiKeyId::new()),
        project_id: Some(ProjectId::new()),
    };
    let cmd = CancelDispatch {
        actor: ActorContext {
            org_id: dispatch_actor.org_id,
            user_id: UserId::new(),
            team_id: dispatch_actor.team_id,
            api_key_id: dispatch_actor.api_key_id,
            project_id: dispatch_actor.project_id,
        },
        dispatch_id: DispatchId::new(),
        reason: Some("requested".to_owned()),
    };
    let decision = engine
        .check_cancel_allowed(&cmd, &dispatch_actor)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Allowed);
    assert_eq!(decision.reason_code, None);
}

#[test]
fn cancel_denied_when_org_mismatches() {
    let engine = PolicyEngine::new();
    let dispatch_actor = ActorContext::new(OrgId::new(), UserId::new());
    let cmd = CancelDispatch {
        actor: ActorContext::new(OrgId::new(), UserId::new()),
        dispatch_id: DispatchId::new(),
        reason: None,
    };
    let decision = engine
        .check_cancel_allowed(&cmd, &dispatch_actor)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::CancelOrgMismatch)
    );
}

#[test]
fn cancel_denied_when_project_scope_mismatches() {
    let engine = PolicyEngine::new();
    let dispatch_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: Some(ProjectId::new()),
    };
    let cmd = CancelDispatch {
        actor: ActorContext {
            org_id: dispatch_actor.org_id,
            user_id: UserId::new(),
            team_id: None,
            api_key_id: None,
            project_id: Some(ProjectId::new()),
        },
        dispatch_id: DispatchId::new(),
        reason: None,
    };
    let decision = engine
        .check_cancel_allowed(&cmd, &dispatch_actor)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::CancelProjectScopeMismatch)
    );
}

#[test]
fn cancel_denied_when_team_scope_mismatches() {
    let engine = PolicyEngine::new();
    let dispatch_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: Some(TeamId::new()),
        api_key_id: None,
        project_id: None,
    };
    let cmd = CancelDispatch {
        actor: ActorContext {
            org_id: dispatch_actor.org_id,
            user_id: UserId::new(),
            team_id: Some(TeamId::new()),
            api_key_id: None,
            project_id: None,
        },
        dispatch_id: DispatchId::new(),
        reason: None,
    };
    let decision = engine
        .check_cancel_allowed(&cmd, &dispatch_actor)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::CancelTeamScopeMismatch)
    );
}

#[test]
fn cancel_denied_when_api_key_scope_mismatches() {
    let engine = PolicyEngine::new();
    let dispatch_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: None,
        api_key_id: Some(ApiKeyId::new()),
        project_id: None,
    };
    let cmd = CancelDispatch {
        actor: ActorContext {
            org_id: dispatch_actor.org_id,
            user_id: UserId::new(),
            team_id: None,
            api_key_id: Some(ApiKeyId::new()),
            project_id: None,
        },
        dispatch_id: DispatchId::new(),
        reason: None,
    };
    let decision = engine
        .check_cancel_allowed(&cmd, &dispatch_actor)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::CancelApiKeyScopeMismatch)
    );
}

#[test]
fn read_allowed_when_scope_matches_dispatch() {
    let engine = PolicyEngine::new();
    let dispatch_id = DispatchId::new();
    let dispatch_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: Some(TeamId::new()),
        api_key_id: Some(ApiKeyId::new()),
        project_id: Some(ProjectId::new()),
    };
    let caller = ActorContext {
        org_id: dispatch_actor.org_id,
        user_id: UserId::new(),
        team_id: dispatch_actor.team_id,
        api_key_id: dispatch_actor.api_key_id,
        project_id: dispatch_actor.project_id,
    };
    let decision = engine
        .check_dispatch_read_allowed(&caller, &dispatch_actor, dispatch_id)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Allowed);
    assert_eq!(decision.reason_code, None);
}

#[test]
fn read_denied_when_org_mismatches() {
    let engine = PolicyEngine::new();
    let dispatch_id = DispatchId::new();
    let dispatch_actor = ActorContext::new(OrgId::new(), UserId::new());
    let caller = ActorContext::new(OrgId::new(), UserId::new());
    let decision = engine
        .check_dispatch_read_allowed(&caller, &dispatch_actor, dispatch_id)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::ReadOrgMismatch)
    );
}

#[test]
fn read_denied_when_project_scope_mismatches() {
    let engine = PolicyEngine::new();
    let dispatch_id = DispatchId::new();
    let dispatch_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: Some(ProjectId::new()),
    };
    let caller = ActorContext {
        org_id: dispatch_actor.org_id,
        user_id: UserId::new(),
        team_id: None,
        api_key_id: None,
        project_id: Some(ProjectId::new()),
    };
    let decision = engine
        .check_dispatch_read_allowed(&caller, &dispatch_actor, dispatch_id)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::ReadProjectScopeMismatch)
    );
}

#[test]
fn read_denied_when_team_scope_mismatches() {
    let engine = PolicyEngine::new();
    let dispatch_id = DispatchId::new();
    let dispatch_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: Some(TeamId::new()),
        api_key_id: None,
        project_id: None,
    };
    let caller = ActorContext {
        org_id: dispatch_actor.org_id,
        user_id: UserId::new(),
        team_id: Some(TeamId::new()),
        api_key_id: None,
        project_id: None,
    };
    let decision = engine
        .check_dispatch_read_allowed(&caller, &dispatch_actor, dispatch_id)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::ReadTeamScopeMismatch)
    );
}

#[test]
fn read_denied_when_api_key_scope_mismatches() {
    let engine = PolicyEngine::new();
    let dispatch_id = DispatchId::new();
    let dispatch_actor = ActorContext {
        org_id: OrgId::new(),
        user_id: UserId::new(),
        team_id: None,
        api_key_id: Some(ApiKeyId::new()),
        project_id: None,
    };
    let caller = ActorContext {
        org_id: dispatch_actor.org_id,
        user_id: UserId::new(),
        team_id: None,
        api_key_id: Some(ApiKeyId::new()),
        project_id: None,
    };
    let decision = engine
        .check_dispatch_read_allowed(&caller, &dispatch_actor, dispatch_id)
        .expect("policy should not error");
    assert_eq!(decision.outcome, PolicyOutcome::Denied);
    assert_eq!(
        decision.reason_code,
        Some(PolicyReasonCode::ReadApiKeyScopeMismatch)
    );
}
