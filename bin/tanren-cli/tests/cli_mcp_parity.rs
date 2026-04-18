use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::prelude::*;
use chrono::{TimeZone, Utc};
use serde_json::{Value, json};
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::{MethodologyEvent, TaskCreated};
use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};
use tanren_domain::{EventId, NonEmptyString, SpecId, TaskId};
use tanren_store::{EventFilter, EventStore, Store};
use tempfile::TempDir;
use uuid::Uuid;

fn fixed_spec_id() -> SpecId {
    SpecId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-0000000000a1").expect("uuid"))
}

fn fixed_task_id() -> TaskId {
    TaskId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-0000000000b2").expect("uuid"))
}

fn fixed_ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5)
        .single()
        .expect("valid timestamp")
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

async fn seed_task(url: &str) {
    let store = Store::new(url).await.expect("open store");
    let task = Task {
        id: fixed_task_id(),
        spec_id: fixed_spec_id(),
        title: NonEmptyString::try_new("seed-task").expect("non-empty"),
        description: String::new(),
        acceptance_criteria: vec![],
        origin: TaskOrigin::User,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: fixed_ts(),
        updated_at: fixed_ts(),
    };
    let envelope = EventEnvelope {
        schema_version: tanren_domain::events::SCHEMA_VERSION,
        event_id: EventId::new(),
        timestamp: fixed_ts(),
        entity_ref: tanren_domain::EntityRef::Task(fixed_task_id()),
        payload: DomainEvent::Methodology {
            event: MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(task),
                origin: TaskOrigin::User,
                idempotency_key: None,
            }),
        },
    };
    store
        .append_methodology_event(&envelope)
        .await
        .expect("seed task");
}

fn run_cli_sequence(url: &str) {
    let task_id = fixed_task_id().to_string();
    for body in [
        json!({"schema_version":"1.0.0","task_id": task_id}),
        json!({"schema_version":"1.0.0","task_id": task_id, "evidence_refs":[]}),
        json!({"schema_version":"1.0.0","task_id": task_id, "guard":"gate_checked"}),
        json!({"schema_version":"1.0.0","task_id": task_id, "guard":"audited"}),
        json!({"schema_version":"1.0.0","task_id": task_id, "guard":"adherent"}),
    ] {
        let sub = if body.get("evidence_refs").is_some() {
            "complete"
        } else if body.get("guard").is_some() {
            "guard"
        } else {
            "start"
        };
        let out = Command::cargo_bin("tanren-cli")
            .expect("bin")
            .env("TANREN_CAPABILITY_OVERRIDE", "admin")
            .args([
                "--database-url",
                url,
                "methodology",
                "task",
                sub,
                "--json",
                &body.to_string(),
            ])
            .output()
            .expect("cli command");
        assert!(
            out.status.success(),
            "cli {sub} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn init_frames() -> Vec<Value> {
    vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "parity-test", "version": "0" }
            }
        }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ]
}

fn send_frame(child: &mut std::process::Child, frame: &Value) {
    let stdin = child.stdin.as_mut().expect("stdin");
    writeln!(stdin, "{frame}").expect("write frame");
    stdin.flush().expect("flush");
}

fn read_response_for_id<R: BufRead>(reader: &mut R, id: i64) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(line.trim())
                    && v["id"] == json!(id)
                {
                    return v;
                }
            }
        }
    }
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "message": format!("timed out waiting for response id={id}") }
    })
}

fn run_mcp_sequence(url: &str) {
    let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
    let mut child = Command::new(&bin)
        .env("TANREN_DATABASE_URL", url)
        .env(
            "TANREN_PHASE_CAPABILITIES",
            "task.start,task.complete,task.read",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tanren-mcp");

    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    for frame in init_frames() {
        send_frame(&mut child, &frame);
    }
    let init = read_response_for_id(&mut reader, 1);
    assert!(
        init.get("error").is_none(),
        "mcp initialize failed: {init:?}"
    );

    let task_id = fixed_task_id().to_string();
    for (id, args) in [
        (2, json!({"schema_version":"1.0.0","task_id": task_id})),
        (
            3,
            json!({"schema_version":"1.0.0","task_id": task_id, "evidence_refs":[]}),
        ),
        (
            4,
            json!({"schema_version":"1.0.0","task_id": task_id, "guard":"gate_checked"}),
        ),
        (
            5,
            json!({"schema_version":"1.0.0","task_id": task_id, "guard":"audited"}),
        ),
        (
            6,
            json!({"schema_version":"1.0.0","task_id": task_id, "guard":"adherent"}),
        ),
    ] {
        let name = match id {
            2 => "start_task",
            3 => "complete_task",
            _ => "mark_task_guard_satisfied",
        };
        send_frame(
            &mut child,
            &json!({
                "jsonrpc":"2.0",
                "id": id,
                "method":"tools/call",
                "params":{"name":name,"arguments":args}
            }),
        );
        let resp = read_response_for_id(&mut reader, id);
        assert_eq!(
            resp["result"]["isError"],
            json!(false),
            "mcp call id={id} failed: {resp:?}"
        );
    }
    let _ = child.kill();
    let _ = child.wait();
}

async fn methodology_payloads(url: &str) -> Vec<Value> {
    let store = Store::new(url).await.expect("open store");
    let mut out = Vec::new();
    let mut cursor = None;
    loop {
        let page = store
            .query_events(&EventFilter {
                event_type: Some("methodology".into()),
                limit: 256,
                cursor,
                ..EventFilter::new()
            })
            .await
            .expect("query events");
        for env in page.events {
            if let DomainEvent::Methodology { event } = env.payload {
                out.push(serde_json::to_value(event).expect("serialize event"));
            }
        }
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor;
    }
    out
}

#[tokio::test]
async fn cli_and_mcp_emit_identical_methodology_event_payload_sequences() {
    let (_d1, cli_url) = mkdb("cli");
    let (_d2, mcp_url) = mkdb("mcp");
    seed_task(&cli_url).await;
    seed_task(&mcp_url).await;

    run_cli_sequence(&cli_url);
    run_mcp_sequence(&mcp_url);

    let cli_events = methodology_payloads(&cli_url).await;
    let mcp_events = methodology_payloads(&mcp_url).await;
    assert_eq!(
        cli_events, mcp_events,
        "CLI and MCP must emit identical methodology event payload sequences for the same scenario"
    );
}
