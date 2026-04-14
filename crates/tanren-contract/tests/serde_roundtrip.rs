//! Serde round-trip tests for all contract types.
//!
//! Ensures every request, response, and error type can serialize to JSON
//! and deserialize back without data loss.

use std::collections::HashMap;

use chrono::Utc;
use tanren_contract::{
    CancelDispatchRequest, ContractError, CreateDispatchRequest, DispatchListFilter,
    DispatchListResponse, DispatchResponse, ErrorResponse, StepResponse,
};
use tanren_domain::{
    AuthMode, Cli, CreateDispatch, DispatchMode, DispatchStatus, Lane, Outcome, Phase,
};
use uuid::Uuid;

fn sample_create_request() -> CreateDispatchRequest {
    CreateDispatchRequest {
        org_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        project: "my-project".to_owned(),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        branch: "main".to_owned(),
        spec_folder: "spec".to_owned(),
        workflow_id: "wf-1".to_owned(),
        mode: DispatchMode::Manual,
        timeout_secs: 300,
        environment_profile: "default".to_owned(),
        team_id: None,
        api_key_id: None,
        project_id: None,
        auth_mode: None,
        gate_cmd: None,
        context: None,
        model: None,
        project_env: None,
        required_secrets: None,
        preserve_on_failure: None,
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
        outcome: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// -- CreateDispatchRequest ---------------------------------------------------

#[test]
fn create_dispatch_request_roundtrip() {
    let req = sample_create_request();
    let json = serde_json::to_string(&req).expect("serialize");
    let back: CreateDispatchRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req, back);
}

#[test]
fn create_dispatch_request_all_optional_fields() {
    let mut req = sample_create_request();
    req.team_id = Some(Uuid::now_v7());
    req.api_key_id = Some(Uuid::now_v7());
    req.project_id = Some(Uuid::now_v7());
    req.auth_mode = Some(AuthMode::OAuth);
    req.gate_cmd = Some("gate check".to_owned());
    req.context = Some("test context".to_owned());
    req.model = Some("claude-4".to_owned());
    req.project_env = Some(HashMap::from([("KEY".to_owned(), "value".to_owned())]));
    req.required_secrets = Some(vec!["SECRET_1".to_owned()]);
    req.preserve_on_failure = Some(true);

    let json = serde_json::to_string(&req).expect("serialize");
    let back: CreateDispatchRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req, back);
}

#[test]
fn create_dispatch_request_snake_case_fields() {
    let req = sample_create_request();
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(json.contains("\"org_id\""), "expected org_id field");
    assert!(json.contains("\"user_id\""), "expected user_id field");
    assert!(
        json.contains("\"timeout_secs\""),
        "expected timeout_secs field"
    );
    assert!(
        json.contains("\"spec_folder\""),
        "expected spec_folder field"
    );
    assert!(
        json.contains("\"environment_profile\""),
        "expected environment_profile field"
    );
}

#[test]
fn enum_fields_serialize_as_snake_case_strings() {
    let req = sample_create_request();
    let json = serde_json::to_string(&req).expect("serialize");
    assert!(
        json.contains("\"do_task\""),
        "Phase::DoTask should serialize as \"do_task\""
    );
    assert!(
        json.contains("\"claude\""),
        "Cli::Claude should serialize as \"claude\""
    );
    assert!(
        json.contains("\"manual\""),
        "DispatchMode::Manual should serialize as \"manual\""
    );
}

// -- DispatchListFilter ------------------------------------------------------

#[test]
fn dispatch_list_filter_roundtrip() {
    let filter = DispatchListFilter {
        status: Some(DispatchStatus::Running),
        lane: Some(Lane::Impl),
        project: Some("proj".to_owned()),
        limit: Some(50),
        offset: Some(10),
    };
    let json = serde_json::to_string(&filter).expect("serialize");
    let back: DispatchListFilter = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(filter, back);
}

#[test]
fn dispatch_list_filter_default_is_empty() {
    let filter = DispatchListFilter::default();
    let json = serde_json::to_string(&filter).expect("serialize");
    assert_eq!(json, "{}");
}

// -- CancelDispatchRequest ---------------------------------------------------

#[test]
fn cancel_dispatch_request_roundtrip() {
    let req = CancelDispatchRequest {
        dispatch_id: Uuid::now_v7(),
        org_id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        team_id: Some(Uuid::now_v7()),
        reason: Some("testing".to_owned()),
    };
    let json = serde_json::to_string(&req).expect("serialize");
    let back: CancelDispatchRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req, back);
}

// -- DispatchResponse --------------------------------------------------------

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

// -- DispatchListResponse ----------------------------------------------------

#[test]
fn dispatch_list_response_roundtrip() {
    let resp = DispatchListResponse {
        dispatches: vec![sample_dispatch_response()],
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    let back: DispatchListResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resp, back);
}

// -- StepResponse ------------------------------------------------------------

#[test]
fn step_response_roundtrip() {
    let resp = StepResponse {
        step_id: Uuid::now_v7(),
        dispatch_id: Uuid::now_v7(),
        step_type: "execute".to_owned(),
        step_sequence: 1,
        lane: Some("impl".to_owned()),
        status: "pending".to_owned(),
        ready_state: "ready".to_owned(),
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

// -- ErrorResponse -----------------------------------------------------------

#[test]
fn error_response_roundtrip() {
    let resp = ErrorResponse {
        code: "not_found".to_owned(),
        message: "dispatch not found".to_owned(),
        details: None,
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    let back: ErrorResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resp, back);
}

#[test]
fn error_response_with_details() {
    let resp = ErrorResponse {
        code: "invalid_input".to_owned(),
        message: "field failed".to_owned(),
        details: Some(serde_json::json!({"field": "project", "hint": "must not be empty"})),
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    let back: ErrorResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(resp, back);
}

// -- ContractError → ErrorResponse mapping -----------------------------------

#[test]
fn contract_error_maps_to_error_response() {
    let err = ContractError::InvalidField {
        field: "project".to_owned(),
        reason: "must not be empty".to_owned(),
    };
    let resp = ErrorResponse::from(err);
    assert_eq!(resp.code, "invalid_input");
    assert!(resp.message.contains("project"));
}

#[test]
fn not_found_maps_correctly() {
    let err = ContractError::NotFound {
        entity: "dispatch".to_owned(),
        id: "abc-123".to_owned(),
    };
    let resp = ErrorResponse::from(err);
    assert_eq!(resp.code, "not_found");
}

#[test]
fn policy_denied_maps_correctly() {
    let err = ContractError::PolicyDenied {
        reason: "budget exceeded".to_owned(),
    };
    let resp = ErrorResponse::from(err);
    assert_eq!(resp.code, "policy_denied");
}

// -- TryFrom<CreateDispatchRequest> for CreateDispatch -----------------------

#[test]
fn valid_request_converts_to_command() {
    let req = sample_create_request();
    let cmd = CreateDispatch::try_from(req);
    assert!(cmd.is_ok());
    let cmd = cmd.expect("should convert");
    assert_eq!(cmd.project.as_str(), "my-project");
    assert_eq!(cmd.timeout.get(), 300);
    assert_eq!(cmd.phase, Phase::DoTask);
    assert_eq!(cmd.cli, Cli::Claude);
    assert_eq!(cmd.mode, DispatchMode::Manual);
}

#[test]
fn empty_project_rejected() {
    let mut req = sample_create_request();
    req.project = String::new();
    let err = CreateDispatch::try_from(req).expect_err("should fail");
    assert!(
        matches!(&err, ContractError::InvalidField { field, .. } if field == "project"),
        "expected InvalidField for project, got: {err:?}"
    );
}

#[test]
fn zero_timeout_rejected() {
    let mut req = sample_create_request();
    req.timeout_secs = 0;
    let err = CreateDispatch::try_from(req).expect_err("should fail");
    assert!(
        matches!(&err, ContractError::InvalidField { field, .. } if field == "timeout_secs"),
        "expected InvalidField for timeout_secs, got: {err:?}"
    );
}

#[test]
fn default_auth_mode_is_api_key() {
    let req = sample_create_request();
    let cmd = CreateDispatch::try_from(req).expect("should convert");
    assert_eq!(cmd.auth_mode, AuthMode::ApiKey);
}

#[test]
fn explicit_auth_mode_preserved() {
    let mut req = sample_create_request();
    req.auth_mode = Some(AuthMode::OAuth);
    let cmd = CreateDispatch::try_from(req).expect("should convert");
    assert_eq!(cmd.auth_mode, AuthMode::OAuth);
}

#[test]
fn project_env_converted() {
    let mut req = sample_create_request();
    req.project_env = Some(HashMap::from([
        ("KEY1".to_owned(), "val1".to_owned()),
        ("KEY2".to_owned(), "val2".to_owned()),
    ]));
    let cmd = CreateDispatch::try_from(req).expect("should convert");
    assert!(!cmd.project_env.is_empty());
}
