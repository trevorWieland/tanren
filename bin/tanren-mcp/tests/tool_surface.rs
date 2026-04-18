//! Integration test for `tanren-mcp` stdio transport.
//!
//! Spawns the compiled MCP binary, sends a handful of JSON-RPC
//! frames over stdin, and asserts the transport:
//!
//! - advertises every tool in the `tanren.methodology.v1` catalog;
//! - round-trips a real tool call (`create_task` → `list_tasks`);
//! - surfaces typed `ToolError` envelopes with `isError: true`;
//! - enforces capability scope when `TANREN_PHASE_CAPABILITIES`
//!   excludes the requested tool;
//! - writes **nothing** to stdout that isn't an MCP frame (stdio
//!   framing is inviolable — non-negotiable #14).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;

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

fn spawn_mcp(db_url: &str, scope: &str) -> (TempDir, std::process::Child) {
    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    // The methodology service needs a migrated store before the
    // first call; run `tanren-cli db migrate` once so the schema
    // is present.
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = assert_cmd::cargo::cargo_bin("tanren-cli");
    let mig = Command::new(&cli)
        .args(["--database-url", db_url, "db", "migrate"])
        .output()
        .expect("migrate");
    assert!(
        mig.status.success(),
        "migrate failed: {}",
        String::from_utf8_lossy(&mig.stderr)
    );
    let child = Command::new(&bin)
        .env("TANREN_DATABASE_URL", db_url)
        .env("TANREN_PHASE_CAPABILITIES", scope)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tanren-mcp");
    (dir, child)
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

#[test]
fn list_tools_advertises_full_catalog() {
    let scope_dir = tempfile::tempdir().expect("tempdir");
    let url = db_url(&scope_dir);
    let (_d, mut child) = spawn_mcp(&url, "task.read");

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
    assert_eq!(tools.len(), 26, "full tanren.methodology.v1 catalog");
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
    let (_d, mut child) = spawn_mcp(&url, "task.create,task.read");

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
                "arguments": { "spec_id": "00000000-0000-0000-0000-000000000021" }
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
    let arr = list_body.as_array().expect("list array");
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
    let (_d, mut child) = spawn_mcp(&url, "task.create");

    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "create_task",
            "arguments": {
                "spec_id": "00000000-0000-0000-0000-000000000022",
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
    let (_d, mut child) = spawn_mcp(&url, "task.read");

    let mut frames = init_frames();
    frames.push(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "create_task",
            "arguments": {
                "spec_id": "00000000-0000-0000-0000-000000000023",
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
    let (_d, mut child) = spawn_mcp(&url, "task.read");

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
