//! CLI auth-boundary hardening tests.

mod support;

use serde_json::Value;
use support::auth::{
    add_auth_args, add_auth_args_with_ttl, assert_stderr_is_single_json, auth_harness,
    claims_missing_jti, sign_with_kid, temp_db,
};

#[test]
fn invalid_actor_token_error_is_generic_without_verification_details() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    support::auth::lint_anchor(&auth);
    let mut cmd = support::auth::cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch-read",
        "list",
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        "wrong-issuer",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
    let message = v["message"].as_str().expect("message");
    assert!(message.contains("token validation failed"));
    assert!(!message.contains("InvalidIssuer"));
    assert!(!message.contains("invalid issuer"));
    assert!(!message.contains("audience"));
    assert!(!message.contains("expired"));
    assert!(!message.contains("signature"));
}

#[test]
fn actor_token_file_read_failure_is_generic_without_path_or_io_details() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let missing_token_path = auth
        .actor_token_file
        .with_file_name("missing-actor-token.jwt");
    let missing_token_text = missing_token_path.display().to_string();
    let mut cmd = support::auth::cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch-read",
        "list",
        "--actor-token-file",
        missing_token_path.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        "tanren-tests",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
    let message = v["message"].as_str().expect("message");
    assert!(message.contains("invalid actor token source"));
    assert!(!message.contains(&missing_token_text));
    assert!(!message.contains("No such file"));
    assert!(!message.contains("os error"));
}

#[test]
fn oversized_actor_token_file_is_rejected_with_generic_error() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    std::fs::write(&auth.actor_token_file, "x".repeat((16 * 1024) + 1)).expect("write token");

    let mut cmd = support::auth::cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch-read",
        "list",
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        "tanren-tests",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "oversized token file must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
    let message = v["message"].as_str().expect("message");
    assert!(message.contains("invalid actor token source"));
}

#[test]
fn actor_public_key_file_read_failure_is_generic_without_path_or_io_details() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let missing_path = auth
        .actor_public_key_file
        .with_file_name("missing-actor-public-key.pem");
    let missing_text = missing_path.display().to_string();
    let mut cmd = support::auth::cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch-read",
        "list",
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        missing_path.to_str().expect("utf8 path"),
        "--token-issuer",
        "tanren-tests",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
    let message = v["message"].as_str().expect("message");
    assert!(message.contains("invalid actor public key"));
    assert!(!message.contains(&missing_text));
    assert!(!message.contains("No such file"));
    assert!(!message.contains("os error"));
}

#[test]
fn actor_public_key_parse_failure_is_generic_without_parser_details() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    std::fs::write(&auth.actor_public_key_file, "not-a-valid-public-key")
        .expect("write invalid key");
    let mut cmd = support::auth::cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch-read",
        "list",
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        "tanren-tests",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
    let message = v["message"].as_str().expect("message");
    assert!(message.contains("invalid actor public key"));
    assert!(!message.contains("Ed25519"));
    assert!(!message.contains("base64"));
}

#[test]
fn token_without_kid_header_is_accepted_with_static_public_key() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let claims = support::auth::base_claims();
    let token = sign_with_kid(&claims, None);
    std::fs::write(&auth.actor_token_file, token).expect("write token");

    let mut cmd = support::auth::cli();
    cmd.args(["--database-url", &db_url, "dispatch-read", "list"]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(
        output.status.success(),
        "missing kid should still authenticate"
    );
}

#[test]
fn token_missing_jti_claim_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let token = sign_with_kid(&claims_missing_jti(), Some("kid-1"));
    std::fs::write(&auth.actor_token_file, token).expect("write token");

    let mut cmd = support::auth::cli();
    cmd.args(["--database-url", &db_url, "dispatch-read", "list"]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
    let message = v["message"].as_str().expect("message");
    assert!(message.contains("token validation failed"));
}

#[test]
fn read_commands_verify_only_allow_immediate_token_reuse() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    support::auth::migrate(&db_url);

    let mut first = support::auth::cli();
    first.args(["--database-url", &db_url, "dispatch-read", "list"]);
    add_auth_args(&mut first, &auth);
    let first_output = first.output().expect("first execute");
    assert!(first_output.status.success(), "first call should pass");

    let mut second = support::auth::cli();
    second.args(["--database-url", &db_url, "dispatch-read", "list"]);
    add_auth_args(&mut second, &auth);
    let second_output = second.output().expect("second execute");
    assert!(second_output.status.success(), "second read should pass");
}

#[test]
fn mutating_commands_consume_replay_and_reject_second_use() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    support::auth::migrate(&db_url);

    let mut first = support::auth::cli();
    first.args([
        "--database-url",
        &db_url,
        "dispatch-mutation",
        "create",
        "--project",
        "test-project",
        "--phase",
        "do_task",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-1",
    ]);
    add_auth_args(&mut first, &auth);
    let first_output = first.output().expect("first execute");
    assert!(
        first_output.status.success(),
        "first create should pass. stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    let mut second = support::auth::cli();
    second.args([
        "--database-url",
        &db_url,
        "dispatch-mutation",
        "create",
        "--project",
        "test-project",
        "--phase",
        "do_task",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-1",
    ]);
    add_auth_args(&mut second, &auth);
    let second_output = second.output().expect("second execute");
    assert!(
        !second_output.status.success(),
        "second create must fail replay"
    );

    let stderr = String::from_utf8(second_output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(
        v["message"]
            .as_str()
            .expect("message")
            .contains("token validation failed")
    );
}

#[test]
fn invalid_token_fails_before_read_store_open() {
    let auth = auth_harness();
    let mut cmd = support::auth::cli();
    cmd.args([
        "--database-url",
        "sqlite:/dev/null/tanren.db?mode=rwc",
        "dispatch-read",
        "list",
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        "wrong-issuer",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "invalid token should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
}

#[test]
fn invalid_token_fails_before_write_store_open_and_migrate() {
    let auth = auth_harness();
    let mut cmd = support::auth::cli();
    cmd.args([
        "--database-url",
        "sqlite:/dev/null/tanren.db?mode=rwc",
        "dispatch-mutation",
        "create",
        "--project",
        "test-project",
        "--phase",
        "do_task",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-1",
        "--actor-token-file",
        auth.actor_token_file.to_str().expect("utf8 path"),
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        "wrong-issuer",
        "--token-audience",
        &auth.audience,
    ]);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "invalid token should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
}

#[test]
fn valid_token_write_backend_failure_maps_to_internal_not_invalid_input() {
    let auth = auth_harness();
    let mut cmd = support::auth::cli();
    cmd.args([
        "--database-url",
        "sqlite:/dev/null/tanren.db?mode=rwc",
        "dispatch-mutation",
        "create",
        "--project",
        "test-project",
        "--phase",
        "do_task",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-1",
    ]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "create should fail on backend");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "internal");
    assert_eq!(v["message"], "internal error");
}

#[test]
fn actor_token_max_ttl_is_enforced() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();

    let mut cmd = support::auth::cli();
    cmd.args(["--database-url", &db_url, "dispatch-read", "list"]);
    add_auth_args_with_ttl(&mut cmd, &auth, 30);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "ttl should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "invalid_input");
    assert!(
        v["message"]
            .as_str()
            .expect("message")
            .contains("token validation failed")
    );
}

#[test]
fn internal_failure_omits_correlation_id_when_sink_persist_fails() {
    let auth = auth_harness();
    let mut cmd = support::auth::cli();
    cmd.env_remove("TANREN_INTERNAL_ERROR_SINK_PATH");
    cmd.env_remove("XDG_STATE_HOME");
    cmd.env_remove("HOME");
    cmd.args([
        "--database-url",
        "sqlite:/dev/null/tanren.db?mode=rwc",
        "dispatch-read",
        "list",
    ]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v = assert_stderr_is_single_json(&stderr);
    assert_eq!(v["code"], "internal");
    assert!(
        v.get("details")
            .and_then(|details| details.get("correlation_id"))
            .is_none(),
        "correlation_id must be omitted when sink persistence fails"
    );
}
