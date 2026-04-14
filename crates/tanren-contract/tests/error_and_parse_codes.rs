use tanren_contract::{ContractError, ErrorCode, ErrorResponse, parse_project_env_entries};

#[test]
fn invalid_transition_maps_to_typed_code() {
    let err = ContractError::InvalidTransition {
        entity: "dispatch 123".to_owned(),
        from: "running".to_owned(),
        to: "cancelled".to_owned(),
    };
    let resp = ErrorResponse::from(err);
    assert_eq!(resp.code, ErrorCode::InvalidTransition);
}

#[test]
fn contention_conflict_maps_to_typed_code() {
    let err = ContractError::ContentionConflict {
        operation: "cancel_dispatch".to_owned(),
        reason: "contention".to_owned(),
    };
    let resp = ErrorResponse::from(err);
    assert_eq!(resp.code, ErrorCode::ContentionConflict);
}

#[test]
fn parse_project_env_entries_accepts_empty_values() {
    let env = parse_project_env_entries(vec!["KEY=".to_owned()]).expect("parse");
    assert_eq!(env.get("KEY"), Some(&String::new()));
}

#[test]
fn parse_project_env_entries_rejects_malformed_entry() {
    let err = parse_project_env_entries(vec!["INVALID".to_owned()]).expect_err("should fail");
    assert!(matches!(
        err,
        ContractError::InvalidField { ref field, .. } if field == "project_env"
    ));
}

#[test]
fn parse_project_env_entries_rejects_duplicate_keys() {
    let err = parse_project_env_entries(vec!["A=1".to_owned(), "A=2".to_owned()])
        .expect_err("should fail");
    assert!(matches!(
        err,
        ContractError::InvalidField { ref field, .. } if field == "project_env"
    ));
}
