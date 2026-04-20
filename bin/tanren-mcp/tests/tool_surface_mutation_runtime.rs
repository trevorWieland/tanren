//! Focused mutation-runtime guard tests for `tanren-mcp`.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

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

fn configure_signed_capability_env(
    command: &mut Command,
    phase: &str,
    spec_id: &str,
    capabilities_csv: &str,
) {
    let spec_id = Uuid::parse_str(spec_id).expect("valid spec uuid");
    let token = mcp_capability_envelope::signed_capability_token(
        phase,
        spec_id,
        "mcp-test-session",
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

fn spawn_with_runtime_spec_folder(
    db_url: &str,
    phase: &str,
    capabilities_csv: &str,
    spec_id: &str,
) -> (TempDir, std::path::PathBuf, std::process::Child) {
    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    let dir = tempfile::tempdir().expect("tempdir");
    migrate_db(db_url);
    let spec_folder = dir
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-runtime-guard"));
    std::fs::create_dir_all(&spec_folder).expect("mkdir");
    let mut command = Command::new(&bin);
    command
        .env("TANREN_DATABASE_URL", db_url)
        .env("TANREN_SPEC_FOLDER", &spec_folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_signed_capability_env(&mut command, phase, spec_id, capabilities_csv);
    let child = command.spawn().expect("spawn tanren-mcp");
    (dir, spec_folder, child)
}

fn spawn_without_runtime_spec_folder(
    db_url: &str,
    phase: &str,
    capabilities_csv: &str,
    spec_id: &str,
) -> (TempDir, std::process::Child) {
    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    let dir = tempfile::tempdir().expect("tempdir");
    migrate_db(db_url);
    let mut command = Command::new(&bin);
    command
        .env("TANREN_DATABASE_URL", db_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_signed_capability_env(&mut command, phase, spec_id, capabilities_csv);
    let child = command.spawn().expect("spawn tanren-mcp");
    (dir, child)
}

#[test]
fn invalid_mutation_does_not_run_postflight_finalize_side_effects() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let (_d, spec_folder, mut child) = spawn_with_runtime_spec_folder(
        &url,
        "do-task",
        "task.create",
        "00000000-0000-0000-0000-000000000122",
    );
    std::fs::write(spec_folder.join("audit.md"), "not-frontmatter\n").expect("seed malformed");

    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "create_task",
            "arguments": {
                "spec_id": "00000000-0000-0000-0000-000000000122",
                "schema_version": "1.0.0",
                "title": "",
                "description": "",
                "origin": { "kind": "user" },
                "acceptance_criteria": []
            }
        }
    }));
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    send_all(&mut child, &frames);
    let responses = read_responses(&mut reader, 2);
    kill(child);

    let resp = responses
        .iter()
        .find(|r| r["id"] == json!(2))
        .expect("call response");
    assert_eq!(resp["result"]["isError"], json!(true));
    let text = resp["result"]["content"][0]["text"].as_str().expect("text");
    let body: Value = serde_json::from_str(text).expect("typed ToolError body");
    assert_eq!(body["kind"].as_str(), Some("validation_failed"));
    assert_eq!(body["field_path"].as_str(), Some("/title"));
    assert!(
        !spec_folder.join("phase-events.jsonl").exists(),
        "failed mutation must not run postflight finalize/evidence side effects"
    );
}

#[test]
fn mutation_without_runtime_env_returns_env_scoped_validation_error() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let (_d, mut child) = spawn_without_runtime_spec_folder(
        &url,
        "do-task",
        "task.create",
        "00000000-0000-0000-0000-000000000123",
    );

    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "create_task",
            "arguments": {
                "spec_id": "00000000-0000-0000-0000-000000000123",
                "schema_version": "1.0.0",
                "title": "task",
                "description": "",
                "origin": { "kind": "user" },
                "acceptance_criteria": []
            }
        }
    }));
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    send_all(&mut child, &frames);
    let responses = read_responses(&mut reader, 2);
    kill(child);

    let resp = responses
        .iter()
        .find(|r| r["id"] == json!(2))
        .expect("call response");
    assert_eq!(resp["result"]["isError"], json!(true));
    let text = resp["result"]["content"][0]["text"].as_str().expect("text");
    let body: Value = serde_json::from_str(text).expect("typed ToolError body");
    assert_eq!(body["kind"].as_str(), Some("validation_failed"));
    assert_eq!(body["field_path"].as_str(), Some("/env"));
    assert!(
        body["expected"].as_str().is_some_and(|value| {
            value.contains("TANREN_SPEC_ID") && value.contains("TANREN_SPEC_FOLDER")
        }),
        "expected runtime env guidance in validation payload"
    );
}
