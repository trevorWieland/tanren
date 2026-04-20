//! Startup/auth-surface integration tests for `tanren-mcp`.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{path::Path, path::PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

#[path = "../../../tests/support/mcp_capability_envelope.rs"]
mod mcp_capability_envelope;

fn init_frames() -> Vec<Value> {
    vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "integration", "version": "0" }
            }
        }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ]
}

fn send_all(child: &mut std::process::Child, frames: &[Value]) {
    let stdin = child.stdin.as_mut().expect("stdin");
    for f in frames {
        writeln!(stdin, "{f}").expect("write frame");
    }
    stdin.flush().expect("flush");
}

fn read_responses<R: BufRead>(reader: &mut R, count: usize) -> Vec<Value> {
    let mut out = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while out.len() < count {
        if std::time::Instant::now() > deadline {
            break;
        }
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(line.trim())
                    && v.get("id").is_some()
                {
                    out.push(v);
                }
            }
        }
    }
    out
}

fn kill(mut child: std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn db_url(dir: &TempDir) -> String {
    format!("sqlite:{}/mcp.db?mode=rwc", dir.path().display())
}

fn migrate_db(url: &str) {
    let cli = assert_cmd::cargo::cargo_bin("tanren-cli");
    let mig = Command::new(&cli)
        .args(["--database-url", url, "db", "migrate"])
        .output()
        .expect("migrate");
    assert!(
        mig.status.success(),
        "migrate failed: {}",
        String::from_utf8_lossy(&mig.stderr)
    );
}

fn run_mcp_startup_output(
    url: &str,
    spec_folder: &Path,
    token: Option<String>,
    issuer: &str,
    audience: &str,
) -> std::process::Output {
    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    let mut cmd = Command::new(bin);
    cmd.env("TANREN_DATABASE_URL", url)
        .env("TANREN_SPEC_FOLDER", spec_folder)
        .env(
            "TANREN_MCP_CAPABILITY_PUBLIC_KEY_PEM",
            mcp_capability_envelope::test_capability_public_key_pem(),
        )
        .env("TANREN_MCP_CAPABILITY_ISSUER", issuer)
        .env("TANREN_MCP_CAPABILITY_AUDIENCE", audience)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(token) = token {
        cmd.env("TANREN_MCP_CAPABILITY_ENVELOPE", token);
    }
    cmd.output().expect("run tanren-mcp")
}

fn configure_signed_capability_env(
    command: &mut Command,
    phase: &str,
    spec_id: &str,
    agent_session_id: &str,
    capabilities_csv: &str,
) {
    let spec_id = Uuid::parse_str(spec_id).expect("valid spec uuid");
    let token = mcp_capability_envelope::signed_capability_token(
        phase,
        spec_id,
        agent_session_id,
        capabilities_csv,
    );
    command
        .env("TANREN_MCP_CAPABILITY_ENVELOPE", token)
        .env(
            "TANREN_MCP_CAPABILITY_PUBLIC_KEY_PEM",
            mcp_capability_envelope::test_capability_public_key_pem(),
        )
        .env(
            "TANREN_MCP_CAPABILITY_ISSUER",
            mcp_capability_envelope::test_capability_issuer(),
        )
        .env(
            "TANREN_MCP_CAPABILITY_AUDIENCE",
            mcp_capability_envelope::test_capability_audience(),
        );
}

fn spawn_mcp_with_session(
    db_url: &str,
    phase: &str,
    capabilities_csv: &str,
    spec_id: &str,
    agent_session_id: &str,
) -> (TempDir, PathBuf, std::process::Child) {
    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    let dir = tempfile::tempdir().expect("tempdir");
    migrate_db(db_url);
    let spec_folder = dir
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-mcp-test"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir spec folder");
    let mut command = Command::new(&bin);
    command
        .env("TANREN_DATABASE_URL", db_url)
        .env("TANREN_SPEC_FOLDER", &spec_folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_signed_capability_env(
        &mut command,
        phase,
        spec_id,
        agent_session_id,
        capabilities_csv,
    );
    let child = command.spawn().expect("spawn tanren-mcp");
    (dir, spec_folder, child)
}

#[test]
fn tanren_config_env_controls_runtime_settings() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    migrate_db(&url);

    let bad_cfg = scope_dir.path().join("bad.yml");
    std::fs::write(&bad_cfg, "methodology: [").expect("write bad cfg");
    let spec_folder = scope_dir.path().join("spec");
    std::fs::create_dir_all(&spec_folder).expect("mkdir spec folder");
    let token = mcp_capability_envelope::signed_capability_token(
        "do-task",
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("uuid"),
        "mcp-test-session",
        "task.read",
    );

    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    let out = Command::new(bin)
        .env("TANREN_DATABASE_URL", &url)
        .env("TANREN_SPEC_FOLDER", &spec_folder)
        .env("TANREN_CONFIG", &bad_cfg)
        .env("TANREN_MCP_CAPABILITY_ENVELOPE", token)
        .env(
            "TANREN_MCP_CAPABILITY_PUBLIC_KEY_PEM",
            mcp_capability_envelope::test_capability_public_key_pem(),
        )
        .env(
            "TANREN_MCP_CAPABILITY_ISSUER",
            mcp_capability_envelope::test_capability_issuer(),
        )
        .env(
            "TANREN_MCP_CAPABILITY_AUDIENCE",
            mcp_capability_envelope::test_capability_audience(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run tanren-mcp");
    assert!(
        !out.status.success(),
        "invalid TANREN_CONFIG should fail startup"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bad.yml"),
        "stderr should reference TANREN_CONFIG path: {stderr}"
    );
}

#[test]
fn startup_requires_signed_capability_envelope() {
    let root = tempfile::tempdir().expect("tempdir");
    let url = db_url(&root);
    migrate_db(&url);
    let spec_folder = root.path().join("spec-no-token");
    std::fs::create_dir_all(&spec_folder).expect("mkdir spec folder");
    let out = run_mcp_startup_output(
        &url,
        &spec_folder,
        None,
        mcp_capability_envelope::test_capability_issuer(),
        mcp_capability_envelope::test_capability_audience(),
    );
    assert!(
        !out.status.success(),
        "startup without envelope token must fail"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("TANREN_MCP_CAPABILITY_ENVELOPE"),
        "stderr should mention missing token env: {stderr}"
    );
}

#[test]
fn startup_rejects_invalid_or_expired_signed_envelope() {
    let root = tempfile::tempdir().expect("tempdir");
    let url = db_url(&root);
    migrate_db(&url);
    let spec_folder = root.path().join("spec-invalid-token");
    std::fs::create_dir_all(&spec_folder).expect("mkdir spec folder");
    let spec_id = Uuid::parse_str("00000000-0000-0000-0000-000000000111").expect("uuid");
    let valid_token = mcp_capability_envelope::signed_capability_token(
        "do-task",
        spec_id,
        "mcp-test-session",
        "task.read",
    );
    let mut invalid_sig = valid_token.clone();
    invalid_sig.push('x');
    let bad_sig = run_mcp_startup_output(
        &url,
        &spec_folder,
        Some(invalid_sig),
        mcp_capability_envelope::test_capability_issuer(),
        mcp_capability_envelope::test_capability_audience(),
    );
    assert!(
        !bad_sig.status.success(),
        "startup with invalid signature must fail"
    );

    let mut expired_claims = mcp_capability_envelope::CapabilityEnvelopeClaimsFixture::valid(
        "do-task",
        spec_id,
        "mcp-test-session",
        "task.read",
    );
    let now = chrono::Utc::now().timestamp();
    expired_claims.exp = now - 20;
    expired_claims.nbf = now - 40;
    expired_claims.iat = now - 50;
    let expired_token = mcp_capability_envelope::sign_capability_envelope(&expired_claims);
    let expired = run_mcp_startup_output(
        &url,
        &spec_folder,
        Some(expired_token),
        mcp_capability_envelope::test_capability_issuer(),
        mcp_capability_envelope::test_capability_audience(),
    );
    assert!(
        !expired.status.success(),
        "startup with expired claims must fail"
    );

    let wrong_issuer = run_mcp_startup_output(
        &url,
        &spec_folder,
        Some(valid_token),
        "wrong-issuer",
        mcp_capability_envelope::test_capability_audience(),
    );
    assert!(
        !wrong_issuer.status.success(),
        "startup with issuer mismatch must fail"
    );
}

#[test]
fn verified_claims_drive_runtime_phase_spec_and_session_context() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let spec_id = "00000000-0000-0000-0000-000000000026";
    let agent_session = "mcp-claims-session";
    let (_d, spec_folder, mut child) = spawn_mcp_with_session(
        &url,
        "do-task",
        "task.create,task.read",
        spec_id,
        agent_session,
    );

    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "create_task",
            "arguments": {
                "spec_id": spec_id,
                "schema_version": "1.0.0",
                "title": "claims test",
                "description": "",
                "origin": { "kind": "user" },
                "acceptance_criteria": []
            }
        }
    }));
    frames.push(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {
            "name": "list_tasks",
            "arguments": { "schema_version": "1.0.0" }
        }
    }));

    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    send_all(&mut child, &frames);
    let responses = read_responses(&mut reader, 3);
    kill(child);

    let create = responses
        .iter()
        .find(|r| r["id"] == json!(2))
        .expect("create response");
    assert_eq!(create["result"]["isError"], json!(false));
    let list = responses
        .iter()
        .find(|r| r["id"] == json!(3))
        .expect("list response");
    assert_eq!(list["result"]["isError"], json!(false));

    let phase_events =
        std::fs::read_to_string(spec_folder.join("phase-events.jsonl")).expect("read phase events");
    assert!(
        phase_events.contains("\"phase\":\"do-task\""),
        "phase-events must be stamped from verified phase claim"
    );
    assert!(
        phase_events.contains(&format!("\"agent_session_id\":\"{agent_session}\"")),
        "phase-events must be stamped from verified agent_session_id claim"
    );
}
