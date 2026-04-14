//! Serde and conversion round-trip tests for contract request/response types.

use std::collections::HashMap;

use chrono::Utc;
use tanren_contract::{
    AuthMode, CancelDispatchRequest, Cli, ContractError, CreateDispatchRequest,
    DispatchCursorToken, DispatchListFilter, DispatchListResponse, DispatchMode, DispatchResponse,
    DispatchStatus, ErrorCode, ErrorResponse, Lane, Outcome, Phase, StepReadyState, StepResponse,
    StepStatus, StepType, cancel_dispatch_from_request, create_dispatch_from_request,
};
use tanren_domain::{ActorContext, OrgId, UserId};
use uuid::Uuid;

fn sample_actor() -> ActorContext {
    ActorContext::new(OrgId::new(), UserId::new())
}

fn sample_create_request() -> CreateDispatchRequest {
    CreateDispatchRequest {
        project: "my-project".to_owned(),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        branch: "main".to_owned(),
        spec_folder: "spec".to_owned(),
        workflow_id: "wf-1".to_owned(),
        mode: DispatchMode::Manual,
        timeout_secs: 300,
        environment_profile: "default".to_owned(),
        auth_mode: AuthMode::ApiKey,
        gate_cmd: None,
        context: None,
        model: None,
        project_env: HashMap::new(),
        required_secrets: Vec::new(),
        preserve_on_failure: false,
    }
}

fn sample_dispatch_response() -> DispatchResponse {
    DispatchResponse {
        dispatch_id: Uuid::now_v7(),
        status: DispatchStatus::Pending,
        mode: DispatchMode::Manual,
        lane: Lane::Impl,
        project: "my-project".to_owned(),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        branch: "main".to_owned(),
        spec_folder: "spec".to_owned(),
        workflow_id: "wf-1".to_owned(),
        environment_profile: "default".to_owned(),
        timeout_secs: 300,
        auth_mode: AuthMode::ApiKey,
        gate_cmd: Some("gate check".to_owned()),
        context: Some("test context".to_owned()),
        model: Some("claude-4".to_owned()),
        project_env_keys: vec!["KEY".to_owned()],
        required_secrets: vec!["SECRET_1".to_owned()],
        preserve_on_failure: true,
        outcome: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn create_dispatch_request_roundtrip() {
    let req = sample_create_request();
    let json = serde_json::to_string(&req).expect("serialize");
    let back: CreateDispatchRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req, back);
}

#[test]
fn create_dispatch_request_deny_unknown_fields_rejects_legacy_actor_fields() {
    let json = serde_json::json!({
        "project": "my-project",
        "phase": "do_task",
        "cli": "claude",
        "branch": "main",
        "spec_folder": "spec",
        "workflow_id": "wf-1",
        "mode": "manual",
        "timeout_secs": 300,
        "environment_profile": "default",
        "org_id": Uuid::now_v7(),
    });

    let err = serde_json::from_value::<CreateDispatchRequest>(json).expect_err("must fail");
    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn create_dispatch_request_missing_normalized_fields_uses_defaults() {
    let json = serde_json::json!({
        "project": "my-project",
        "phase": "do_task",
        "cli": "claude",
        "branch": "main",
        "spec_folder": "spec",
        "workflow_id": "wf-1",
        "mode": "manual",
        "timeout_secs": 300,
        "environment_profile": "default"
    });
    let req: CreateDispatchRequest = serde_json::from_value(json).expect("deserialize");
    assert_eq!(req.auth_mode, AuthMode::ApiKey);
    assert!(req.project_env.is_empty());
    assert!(req.required_secrets.is_empty());
    assert!(!req.preserve_on_failure);
}

#[test]
fn cancel_dispatch_request_roundtrip() {
    let req = CancelDispatchRequest {
        dispatch_id: Uuid::now_v7(),
        reason: Some("testing".to_owned()),
    };
    let json = serde_json::to_string(&req).expect("serialize");
    let back: CancelDispatchRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req, back);
}

#[test]
fn cancel_dispatch_request_deny_unknown_fields_rejects_legacy_actor_fields() {
    let json = serde_json::json!({
        "dispatch_id": Uuid::now_v7(),
        "reason": "test",
        "user_id": Uuid::now_v7(),
    });
    let err = serde_json::from_value::<CancelDispatchRequest>(json).expect_err("must fail");
    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn dispatch_response_roundtrip() {
    let resp = sample_dispatch_response();
    let json = serde_json::to_string(&resp).expect("serialize");
    let back: DispatchResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resp, back);
}

#[test]
fn dispatch_response_with_outcome() {
    let mut resp = sample_dispatch_response();
    resp.outcome = Some(Outcome::Success);
    let json = serde_json::to_string(&resp).expect("serialize");
    let back: DispatchResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resp, back);
}

#[test]
fn dispatch_list_filter_roundtrip() {
    let filter = DispatchListFilter {
        status: Some(DispatchStatus::Running),
        lane: Some(Lane::Impl),
        project: Some("proj".to_owned()),
        limit: Some(50),
        cursor: Some(
            DispatchCursorToken::decode(
                "v1|2026-01-01T00:00:00+00:00|01966a00-0000-7000-8000-000000000001",
            )
            .expect("cursor"),
        ),
    };
    let json = serde_json::to_string(&filter).expect("serialize");
    let back: DispatchListFilter = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(filter, back);
}

#[test]
fn dispatch_list_response_roundtrip() {
    let resp = DispatchListResponse {
        dispatches: vec![sample_dispatch_response()],
        next_cursor: Some(DispatchCursorToken::new(Utc::now(), Uuid::now_v7())),
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    let back: DispatchListResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resp, back);
}

#[test]
fn step_response_roundtrip() {
    let resp = StepResponse {
        step_id: Uuid::now_v7(),
        dispatch_id: Uuid::now_v7(),
        step_type: StepType::Execute,
        step_sequence: 1,
        lane: Some(Lane::Impl),
        status: StepStatus::Pending,
        ready_state: StepReadyState::Ready,
        worker_id: None,
        error: None,
        retry_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    let back: StepResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resp, back);
}

#[test]
fn error_response_roundtrip() {
    let resp = ErrorResponse {
        code: ErrorCode::NotFound,
        message: "dispatch not found".to_owned(),
        details: None,
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    let back: ErrorResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resp, back);
}

#[test]
fn contract_error_maps_to_error_response() {
    let err = ContractError::InvalidField {
        field: "project".to_owned(),
        reason: "must not be empty".to_owned(),
    };
    let resp = ErrorResponse::from(err);
    assert_eq!(resp.code, ErrorCode::InvalidInput);
    assert!(resp.message.contains("project"));
}

#[test]
fn valid_request_converts_to_domain_command_with_trusted_actor() {
    let req = sample_create_request();
    let actor = sample_actor();
    let cmd = create_dispatch_from_request(actor.clone(), req).expect("convert");

    assert_eq!(cmd.actor, actor);
    assert_eq!(cmd.project.as_str(), "my-project");
    assert_eq!(cmd.timeout.get(), 300);
    assert_eq!(cmd.phase, tanren_domain::Phase::DoTask);
    assert_eq!(cmd.cli, tanren_domain::Cli::Claude);
    assert_eq!(cmd.mode, tanren_domain::DispatchMode::Manual);
}

#[test]
fn empty_project_rejected() {
    let mut req = sample_create_request();
    req.project = String::new();
    let err = create_dispatch_from_request(sample_actor(), req).expect_err("should fail");
    assert!(
        matches!(&err, ContractError::InvalidField { field, .. } if field == "project"),
        "expected InvalidField for project, got: {err:?}"
    );
}

#[test]
fn cancel_request_converts_to_domain_command_with_trusted_actor() {
    let actor = sample_actor();
    let req = CancelDispatchRequest {
        dispatch_id: Uuid::now_v7(),
        reason: Some("because".to_owned()),
    };

    let cmd = cancel_dispatch_from_request(actor.clone(), req.clone()).expect("convert");
    assert_eq!(cmd.actor, actor);
    assert_eq!(cmd.dispatch_id.into_uuid(), req.dispatch_id);
    assert_eq!(cmd.reason, req.reason);
}
