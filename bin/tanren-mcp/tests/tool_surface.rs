//! Integration test for `tanren-mcp` stdio transport.
//!
//! Spawns the compiled MCP binary, sends a handful of JSON-RPC
//! frames over stdin, and asserts the transport:
//!
//! - advertises every tool in the `tanren.methodology.v1` catalog;
//! - round-trips a real tool call (`create_task` → `list_tasks`);
//! - surfaces typed `ToolError` envelopes with `isError: true`;
//! - enforces capability scope when signed claims exclude the
//!   requested tool;
//! - writes **nothing** to stdout that isn't an MCP frame (stdio
//!   framing is inviolable — non-negotiable #14).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{collections::BTreeSet, path::PathBuf};

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

fn spawn_mcp(
    db_url: &str,
    phase: &str,
    capabilities_csv: &str,
    spec_id: &str,
) -> (TempDir, PathBuf, std::process::Child) {
    spawn_mcp_with_session(db_url, phase, capabilities_csv, spec_id, "mcp-test-session")
}

fn spawn_mcp_with_session(
    db_url: &str,
    phase: &str,
    capabilities_csv: &str,
    spec_id: &str,
    agent_session_id: &str,
) -> (TempDir, PathBuf, std::process::Child) {
    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    // The methodology service needs a migrated store before the
    // first call; run `tanren-cli db migrate` once so the schema
    // is present.
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

fn send_all(child: &mut std::process::Child, frames: &[Value]) {
    let stdin = child.stdin.as_mut().expect("stdin");
    for f in frames {
        writeln!(stdin, "{f}").expect("write frame");
    }
    stdin.flush().expect("flush");
}

/// Read JSON-RPC response lines until `count` id-bearing responses
/// have been collected or the pipe closes.
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

fn documented_tool_names() -> BTreeSet<String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let doc = manifest_dir.join("../../docs/architecture/agent-tool-surface.md");
    let text = std::fs::read_to_string(&doc).expect("read agent-tool-surface.md");
    let mut out = BTreeSet::new();
    for line in text.lines() {
        if !line.starts_with('|') || !line.contains('`') {
            continue;
        }
        let mut cursor = 0usize;
        while let Some(start_rel) = line[cursor..].find('`') {
            let start = cursor + start_rel + 1;
            let Some(end_rel) = line[start..].find('`') else {
                break;
            };
            let end = start + end_rel;
            let token = &line[start..end];
            if let Some((name, _)) = token.split_once('(')
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
            {
                out.insert(name.to_owned());
            }
            cursor = end + 1;
        }
    }
    out
}

#[test]
fn list_tools_advertises_full_catalog() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let (_d, _spec_folder, mut child) = spawn_mcp(
        &url,
        "do-task",
        "task.read",
        "00000000-0000-0000-0000-000000000001",
    );

    let mut frames = init_frames();
    frames.push(json!({ "jsonrpc":"2.0", "id":2, "method":"tools/list" }));
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    send_all(&mut child, &frames);
    let responses = read_responses(&mut reader, 2);
    kill(child);

    let list = responses
        .iter()
        .find(|r| r["id"] == json!(2))
        .expect("tools/list response");
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 28, "full tanren.methodology.v1 catalog");
    // Spot-check: every tool carries schema_version metadata.
    for t in tools {
        assert!(t["name"].is_string());
        assert!(t["inputSchema"].is_object());
        assert_eq!(
            t["_meta"]["schema_version"].as_str(),
            Some("1.0.0"),
            "every tool must advertise schema_version"
        );
    }
}

#[test]
fn call_tool_round_trips_create_and_list() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let (_d, _spec_folder, mut child) = spawn_mcp(
        &url,
        "do-task",
        "task.create,task.read",
        "00000000-0000-0000-0000-000000000021",
    );

    // Two-phase send: the rmcp server processes both requests on a
    // single tokio task, but `create_task` returns *before* the
    // store append has flushed to sqlite (write happens inside
    // `emit` but the completion future races the read-side
    // `list_tasks`). Send the create, wait for its reply, then
    // issue `list_tasks` with the store already at rest.
    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "create_task",
            "arguments": {
                "spec_id": "00000000-0000-0000-0000-000000000021",
                "schema_version": "1.0.0",
                "title": "mcp task",
                "description": "",
                "origin": { "kind": "user" },
                "acceptance_criteria": []
            }
        }
    }));
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    send_all(&mut child, &frames);
    let mut responses = read_responses(&mut reader, 2);
    send_all(
        &mut child,
        &[json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {
                "name": "list_tasks",
                "arguments": {
                    "schema_version": "1.0.0",
                    "spec_id": "00000000-0000-0000-0000-000000000021"
                }
            }
        })],
    );
    let phase2 = read_responses(&mut reader, 1);
    responses.extend(phase2);
    kill(child);
    assert_eq!(responses.len(), 3, "expected 3 id-bearing responses");

    let create = responses
        .iter()
        .find(|r| r["id"] == json!(2))
        .expect("create response");
    assert_eq!(create["result"]["isError"], json!(false));
    let create_text = create["result"]["content"][0]["text"]
        .as_str()
        .expect("create content text");
    let create_body: Value = serde_json::from_str(create_text).expect("create body");
    assert_eq!(create_body["schema_version"].as_str(), Some("1.0.0"));
    assert!(create_body["task_id"].is_string());

    let list = responses
        .iter()
        .find(|r| r["id"] == json!(3))
        .expect("list response");
    assert_eq!(list["result"]["isError"], json!(false));
    let list_text = list["result"]["content"][0]["text"]
        .as_str()
        .expect("list content text");
    let list_body: Value = serde_json::from_str(list_text).expect("list body");
    assert_eq!(list_body["schema_version"].as_str(), Some("1.0.0"));
    let arr = list_body["tasks"].as_array().expect("list tasks array");
    assert_eq!(
        arr.len(),
        1,
        "the task we just created must appear. create={create_body:?} list_text={list_text}"
    );
}

#[test]
fn call_tool_with_invalid_params_returns_typed_validation_error() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let (_d, _spec_folder, mut child) = spawn_mcp(
        &url,
        "do-task",
        "task.create",
        "00000000-0000-0000-0000-000000000022",
    );

    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "create_task",
            "arguments": {
                "spec_id": "00000000-0000-0000-0000-000000000022",
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
}

#[test]
fn capability_denied_when_scope_excludes_tool() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    // Scope grants only task.read — create_task must be denied.
    let (_d, _spec_folder, mut child) = spawn_mcp(
        &url,
        "do-task",
        "task.read",
        "00000000-0000-0000-0000-000000000023",
    );

    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "create_task",
            "arguments": {
                "spec_id": "00000000-0000-0000-0000-000000000023",
                "schema_version": "1.0.0",
                "title": "nope",
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
    assert_eq!(body["kind"].as_str(), Some("capability_denied"));
    assert_eq!(body["capability"].as_str(), Some("task.create"));
}

#[test]
fn unknown_tool_returns_typed_not_found() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let (_d, _spec_folder, mut child) = spawn_mcp(
        &url,
        "do-task",
        "task.read",
        "00000000-0000-0000-0000-000000000024",
    );

    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "not_a_real_tool", "arguments": {} }
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
    assert_eq!(body["kind"].as_str(), Some("not_found"));
    assert_eq!(body["resource"].as_str(), Some("tool"));
}

#[test]
fn catalog_and_agent_tool_surface_doc_stay_in_parity() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let (_d, _spec_folder, mut child) = spawn_mcp(
        &url,
        "do-task",
        "task.read",
        "00000000-0000-0000-0000-000000000025",
    );

    let mut frames = init_frames();
    frames.push(json!({ "jsonrpc":"2.0", "id":2, "method":"tools/list" }));
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    send_all(&mut child, &frames);
    let responses = read_responses(&mut reader, 2);
    kill(child);

    let list = responses
        .iter()
        .find(|r| r["id"] == json!(2))
        .expect("tools/list response");
    let tools = list["result"]["tools"].as_array().expect("tools array");
    let runtime: BTreeSet<String> = tools
        .iter()
        .filter_map(|t| t["name"].as_str().map(str::to_owned))
        .collect();
    let documented = documented_tool_names();
    assert_eq!(
        runtime, documented,
        "runtime catalog and docs/architecture/agent-tool-surface.md must match"
    );
}
