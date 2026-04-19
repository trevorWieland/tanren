use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use assert_cmd::prelude::*;
use serde_json::{Value, json};
use tanren_app_services::methodology::PhaseEventLine;
use tanren_contract::methodology as c;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin};
use tanren_domain::{SpecId, TaskId};
use tempfile::TempDir;
use uuid::Uuid;

#[path = "../../../tests/support/methodology_event_parity.rs"]
mod methodology_event_parity;

const MCP_CAPABILITIES: &str = "task.create,task.start,task.complete";

fn init_frames() -> Vec<Value> {
    vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "transport-parity", "version": "0" }
            }
        }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ]
}

fn send_all(child: &mut Child, frames: &[Value]) {
    let stdin = child.stdin.as_mut().expect("stdin");
    for frame in frames {
        writeln!(stdin, "{frame}").expect("write frame");
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
                if let Ok(value) = serde_json::from_str::<Value>(line.trim())
                    && value.get("id").is_some()
                {
                    out.push(value);
                }
            }
        }
    }
    out
}

fn kill(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn schema() -> c::SchemaVersion {
    c::SchemaVersion::current()
}

fn fixed_spec_id() -> SpecId {
    SpecId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-0000000000a1").expect("uuid"))
}

fn mkdb(name: &str) -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join(format!("{name}.db"));
    let url = format!("sqlite:{}?mode=rwc", db.display());
    Command::cargo_bin("tanren-cli")
        .expect("bin")
        .args(["--database-url", &url, "db", "migrate"])
        .assert()
        .success();
    (dir, url)
}

fn run_cli_tool(
    url: &str,
    spec_folder: &Path,
    noun: &str,
    verb: &str,
    params: &impl serde::Serialize,
) -> Value {
    let payload = serde_json::to_string(params).expect("serialize payload");
    let spec_id = fixed_spec_id().to_string();
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .env("TANREN_CAPABILITY_OVERRIDE", "admin")
        .env("TANREN_PHASE_CAPABILITIES", MCP_CAPABILITIES)
        .args([
            "--database-url",
            url,
            "methodology",
            "--phase",
            "do-task",
            "--spec-id",
            spec_id.as_str(),
            "--spec-folder",
            spec_folder.to_str().expect("utf8 path"),
            "--agent-session-id",
            "parity-session",
            noun,
            verb,
            "--json",
            &payload,
        ])
        .output()
        .expect("run cli tool");
    assert!(
        out.status.success(),
        "cli {noun} {verb} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice::<Value>(&out.stdout).expect("parse cli stdout json")
}

fn run_cli_matrix(url: &str, spec_folder: &Path) {
    let spec_id = fixed_spec_id();
    let create: c::CreateTaskResponse = serde_json::from_value(run_cli_tool(
        url,
        spec_folder,
        "task",
        "create",
        &c::CreateTaskParams {
            schema_version: schema(),
            spec_id,
            title: "parity task".into(),
            description: "task description".into(),
            parent_task_id: None,
            depends_on: vec![],
            origin: TaskOrigin::ShapeSpec,
            acceptance_criteria: vec![],
            idempotency_key: Some("parity-create".into()),
        },
    ))
    .expect("create task response");

    let task_id = create.task_id;
    let _ = run_cli_tool(
        url,
        spec_folder,
        "task",
        "start",
        &c::StartTaskParams {
            schema_version: schema(),
            task_id,
            idempotency_key: Some("parity-start".into()),
        },
    );
    let _ = run_cli_tool(
        url,
        spec_folder,
        "task",
        "complete",
        &c::CompleteTaskParams {
            schema_version: schema(),
            task_id,
            evidence_refs: vec!["src/lib.rs".into()],
            idempotency_key: Some("parity-complete".into()),
        },
    );
    let _ = run_cli_tool(
        url,
        spec_folder,
        "task",
        "guard",
        &c::MarkTaskGuardSatisfiedParams {
            schema_version: schema(),
            task_id,
            guard: RequiredGuard::GateChecked,
            idempotency_key: Some("parity-guard".into()),
        },
    );
}

fn spawn_mcp(url: &str, spec_folder: &Path, spec_id: SpecId) -> Child {
    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    Command::new(bin)
        .env("TANREN_DATABASE_URL", url)
        .env("TANREN_SPEC_ID", spec_id.to_string())
        .env("TANREN_SPEC_FOLDER", spec_folder)
        .env("TANREN_MCP_PHASE", "do-task")
        .env("TANREN_AGENT_SESSION_ID", "parity-session")
        .env("TANREN_PHASE_CAPABILITIES", MCP_CAPABILITIES)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tanren-mcp")
}

fn mcp_call(
    child: &mut Child,
    reader: &mut BufReader<std::process::ChildStdout>,
    id: i64,
    name: &str,
    args: &Value,
) -> Value {
    let frame = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": args,
        }
    });
    send_all(child, &[frame]);
    let responses = read_responses(reader, 1);
    let response = responses
        .into_iter()
        .find(|value| value["id"] == json!(id))
        .expect("tool call response");
    assert_eq!(
        response["result"]["isError"],
        json!(false),
        "mcp tool call failed: {response}"
    );
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool content text");
    serde_json::from_str::<Value>(text).expect("parse tool response body")
}

fn run_mcp_matrix(url: &str, spec_folder: &Path) {
    let spec_id = fixed_spec_id();
    let mut child = spawn_mcp(url, spec_folder, spec_id);

    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    send_all(&mut child, &init_frames());
    let _ = read_responses(&mut reader, 1);

    let create = mcp_call(
        &mut child,
        &mut reader,
        2,
        "create_task",
        &json!({
            "spec_id": spec_id,
            "schema_version": "1.0.0",
            "title": "parity task",
            "description": "task description",
            "origin": { "kind": "shape_spec" },
            "acceptance_criteria": [],
            "idempotency_key": "parity-create"
        }),
    );
    let task_id = TaskId::from_uuid(
        Uuid::parse_str(
            create["task_id"]
                .as_str()
                .expect("task_id in create response"),
        )
        .expect("task id uuid"),
    );

    let _ = mcp_call(
        &mut child,
        &mut reader,
        3,
        "start_task",
        &json!({
            "schema_version": "1.0.0",
            "task_id": task_id,
            "idempotency_key": "parity-start"
        }),
    );
    let _ = mcp_call(
        &mut child,
        &mut reader,
        4,
        "complete_task",
        &json!({
            "schema_version": "1.0.0",
            "task_id": task_id,
            "evidence_refs": ["src/lib.rs"],
            "idempotency_key": "parity-complete"
        }),
    );
    let _ = mcp_call(
        &mut child,
        &mut reader,
        5,
        "mark_task_guard_satisfied",
        &json!({
            "schema_version": "1.0.0",
            "task_id": task_id,
            "guard": "gate_checked",
            "idempotency_key": "parity-guard"
        }),
    );

    kill(child);
}

fn reconcile_phase_events(url: &str, spec_folder: &Path) {
    let spec_id = fixed_spec_id().to_string();
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .env("TANREN_CAPABILITY_OVERRIDE", "admin")
        .args([
            "--database-url",
            url,
            "methodology",
            "--spec-id",
            spec_id.as_str(),
            "--spec-folder",
            spec_folder.to_str().expect("utf8 path"),
            "reconcile-phase-events",
        ])
        .output()
        .expect("reconcile phase-events");
    assert!(
        out.status.success(),
        "reconcile-phase-events failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn phase_event_lines(spec_folder: &Path) -> Vec<PhaseEventLine> {
    let path = spec_folder.join("phase-events.jsonl");
    let content = std::fs::read_to_string(&path).expect("read phase-events.jsonl");
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let value = serde_json::from_str::<PhaseEventLine>(line).expect("parse phase-event line");
        let event_id = value.event_id.to_string();
        if seen.insert(event_id) {
            out.push(value);
        }
    }
    out
}

fn phase_event_line_values(lines: &[PhaseEventLine]) -> Vec<Value> {
    lines
        .iter()
        .map(|line| serde_json::to_value(line).expect("phase line json value"))
        .collect()
}

fn methodology_envelopes(lines: &[PhaseEventLine]) -> Vec<EventEnvelope> {
    lines
        .iter()
        .map(|line| {
            EventEnvelope::new(
                line.event_id,
                line.timestamp,
                DomainEvent::Methodology {
                    event: line.payload.clone(),
                },
            )
        })
        .collect()
}

#[test]
fn mcp_matches_cli_for_event_and_phase_event_projection_parity() {
    let (_d1, cli_url) = mkdb("cli-parity");
    let (_d2, mcp_url) = mkdb("mcp-parity");

    let spec_id = fixed_spec_id();
    let cli_root = tempfile::tempdir().expect("tempdir");
    let mcp_root = tempfile::tempdir().expect("tempdir");
    let cli_spec_folder = cli_root
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-cli"));
    let mcp_spec_folder = mcp_root
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-mcp"));
    std::fs::create_dir_all(&cli_spec_folder).expect("mkdir cli folder");
    std::fs::create_dir_all(&mcp_spec_folder).expect("mkdir mcp folder");

    run_cli_matrix(&cli_url, &cli_spec_folder);
    run_mcp_matrix(&mcp_url, &mcp_spec_folder);
    reconcile_phase_events(&cli_url, &cli_spec_folder);
    reconcile_phase_events(&mcp_url, &mcp_spec_folder);

    let cli_lines = phase_event_lines(&cli_spec_folder);
    let mcp_lines = phase_event_lines(&mcp_spec_folder);
    let cli_events = methodology_envelopes(&cli_lines);
    let mcp_events = methodology_envelopes(&mcp_lines);
    assert_eq!(
        cli_events.len(),
        mcp_events.len(),
        "CLI and MCP must produce event-for-event parity"
    );
    methodology_event_parity::assert_event_stream_strict_parity(&cli_events, &mcp_events);

    let cli_phase_values = phase_event_line_values(&cli_lines);
    let mcp_phase_values = phase_event_line_values(&mcp_lines);
    methodology_event_parity::assert_phase_lines_strict_parity(
        &cli_phase_values,
        &mcp_phase_values,
    );

    assert_eq!(
        cli_events.len(),
        cli_lines.len(),
        "every methodology event must project to one phase-events line (CLI)"
    );
    assert_eq!(
        mcp_events.len(),
        mcp_lines.len(),
        "every methodology event must project to one phase-events line (MCP)"
    );

    for env in &cli_events {
        assert!(
            matches!(env.payload, DomainEvent::Methodology { .. }),
            "expected methodology-only stream"
        );
    }
    for env in &mcp_events {
        assert!(
            matches!(env.payload, DomainEvent::Methodology { .. }),
            "expected methodology-only stream"
        );
    }
}
