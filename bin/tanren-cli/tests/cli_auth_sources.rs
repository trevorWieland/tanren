//! Auth token transport + clap display behavior coverage.

mod support;

use serde_json::Value;
use support::auth::{auth_harness, cli, migrate, temp_db};

#[test]
fn actor_token_cli_arg_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    support::auth::lint_anchor(&auth);
    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "--actor-token",
            &auth.token,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(
        v["message"]
            .as_str()
            .expect("msg")
            .contains("--actor-token")
    );
}

#[test]
fn actor_token_can_be_read_from_stdin() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    migrate(&db_url);

    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "--actor-token-stdin",
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        &auth.issuer,
        "--token-audience",
        &auth.audience,
        "dispatch",
        "list",
    ]);
    cmd.write_stdin(format!("{}\n", auth.token));
    let output = cmd.output().expect("execute");
    assert!(output.status.success(), "stdin token should authenticate");
}

#[test]
fn actor_token_can_be_read_from_env() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    migrate(&db_url);

    let output = cli()
        .env("TANREN_ACTOR_TOKEN", &auth.token)
        .args([
            "--database-url",
            &db_url,
            "--actor-public-key-file",
            auth.actor_public_key_file.to_str().expect("utf8 path"),
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(output.status.success(), "env token should authenticate");
}

#[test]
fn token_source_conflict_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "--actor-token-stdin",
            "--actor-token-file",
            auth.actor_token_file.to_str().expect("utf8 path"),
            "--actor-public-key-file",
            auth.actor_public_key_file.to_str().expect("utf8 path"),
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(
        !output.status.success(),
        "conflicting token sources must fail"
    );
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(v["message"].as_str().expect("msg").contains("token source"));
}

#[test]
fn token_source_conflict_env_plus_file_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let output = cli()
        .env("TANREN_ACTOR_TOKEN", &auth.token)
        .args([
            "--database-url",
            &db_url,
            "--actor-token-file",
            auth.actor_token_file.to_str().expect("utf8 path"),
            "--actor-public-key-file",
            auth.actor_public_key_file.to_str().expect("utf8 path"),
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "env+file conflict must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(v["message"].as_str().expect("msg").contains("token source"));
}

#[test]
fn empty_env_token_is_treated_as_absent_for_source_selection() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    migrate(&db_url);

    let output = cli()
        .env("TANREN_ACTOR_TOKEN", "   ")
        .args([
            "--database-url",
            &db_url,
            "--actor-token-file",
            auth.actor_token_file.to_str().expect("utf8 path"),
            "--actor-public-key-file",
            auth.actor_public_key_file.to_str().expect("utf8 path"),
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(
        output.status.success(),
        "empty env token should not create a multi-source conflict"
    );
}

#[test]
fn token_source_conflict_env_plus_stdin_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    let mut cmd = cli();
    cmd.env("TANREN_ACTOR_TOKEN", &auth.token);
    cmd.args([
        "--database-url",
        &db_url,
        "--actor-token-stdin",
        "--actor-public-key-file",
        auth.actor_public_key_file.to_str().expect("utf8 path"),
        "--token-issuer",
        &auth.issuer,
        "--token-audience",
        &auth.audience,
        "dispatch",
        "list",
    ]);
    cmd.write_stdin(format!("{}\n", auth.token));

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "env+stdin conflict must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(v["message"].as_str().expect("msg").contains("token source"));
}

#[test]
fn legacy_actor_jwks_file_flag_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();

    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "--actor-token-file",
            auth.actor_token_file.to_str().expect("utf8 path"),
            "--actor-jwks-file",
            auth.actor_public_key_file.to_str().expect("utf8 path"),
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "legacy jwks-file flag must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(
        v["message"]
            .as_str()
            .expect("msg")
            .contains("--actor-jwks-file")
    );
}

#[test]
fn legacy_actor_jwks_url_flag_is_rejected() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();

    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "--actor-token-file",
            auth.actor_token_file.to_str().expect("utf8 path"),
            "--actor-jwks-url",
            "https://example.com/jwks.json",
            "--token-issuer",
            &auth.issuer,
            "--token-audience",
            &auth.audience,
            "dispatch",
            "list",
        ])
        .output()
        .expect("execute");

    assert!(!output.status.success(), "legacy jwks-url flag must fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "invalid_input");
    assert!(
        v["message"]
            .as_str()
            .expect("msg")
            .contains("--actor-jwks-url")
    );
}

#[test]
fn help_exits_successfully() {
    let output = cli().arg("--help").output().expect("execute");
    assert!(output.status.success(), "help should exit 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("Tanren"));
    assert!(stdout.contains("--database-url"));
}

#[test]
fn version_exits_successfully() {
    let output = cli().arg("--version").output().expect("execute");
    assert!(output.status.success(), "version should exit 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.starts_with("tanren "));
}
