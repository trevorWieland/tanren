use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;
use serde_json::Value;
use tanren_domain::methodology::events::{MethodologyEvent, TaskCreated};
use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};
use tanren_domain::{NonEmptyString, SpecId, TaskId};
use tempfile::TempDir;

pub(super) fn mkdb() -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("tanren.db");
    let url = format!("sqlite:{}?mode=rwc", db.display());
    Command::cargo_bin("tanren-cli")
        .expect("bin")
        .args(["--database-url", &url, "db", "migrate"])
        .assert()
        .success();
    (dir, url)
}

pub(super) fn mk_spec_folder(dir: &TempDir, spec_id: &str) -> PathBuf {
    let path = dir.path().join(format!("2026-01-01-0101-{spec_id}-test"));
    std::fs::create_dir_all(&path).expect("create spec folder");
    path
}

pub(super) fn cli(url: &str) -> Command {
    let mut cmd = Command::cargo_bin("tanren-cli").expect("bin");
    cmd.args(["--database-url", url]);
    cmd.env(
        "TANREN_PHASE_CAPABILITIES",
        "task.create,task.start,task.complete,task.revise,task.abandon,task.read,finding.add,rubric.record,compliance.record,spec.frontmatter,demo.frontmatter,demo.results,signpost.add,signpost.update,phase.outcome,phase.escalate,issue.create,standard.read,adherence.record,feedback.reply",
    );
    cmd
}

pub(super) fn parse_stdout(out: &std::process::Output) -> Value {
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&text).expect("stdout is JSON")
}

pub(super) fn parse_stderr(out: &std::process::Output) -> Value {
    let text = String::from_utf8_lossy(&out.stderr);
    serde_json::from_str(&text).expect("stderr is JSON")
}

pub(super) fn write_phase_events_file(folder: &Path, spec_id: SpecId) -> PathBuf {
    let task = Task {
        id: TaskId::new(),
        spec_id,
        title: NonEmptyString::try_new("replay task").expect("title"),
        description: String::new(),
        acceptance_criteria: vec![],
        origin: TaskOrigin::User,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let payload = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(task),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let line = serde_json::json!({
        "schema_version": "1.0.0",
        "event_id": uuid::Uuid::now_v7(),
        "spec_id": spec_id,
        "phase": "do-task",
        "agent_session_id": "session-1",
        "timestamp": chrono::Utc::now(),
        "origin_kind": "tool_primary",
        "tool": "create_task",
        "payload": payload,
    });
    let path = folder.join("phase-events.jsonl");
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string(&line).expect("json")),
    )
    .expect("write phase-events");
    path
}

pub(super) fn write_legacy_phase_events_file(folder: &Path, spec_id: SpecId) -> PathBuf {
    let task = Task {
        id: TaskId::new(),
        spec_id,
        title: NonEmptyString::try_new("replay task missing provenance").expect("title"),
        description: String::new(),
        acceptance_criteria: vec![],
        origin: TaskOrigin::User,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let payload = MethodologyEvent::TaskCreated(TaskCreated {
        task: Box::new(task),
        origin: TaskOrigin::User,
        idempotency_key: None,
    });
    let line = serde_json::json!({
        "schema_version": "1.0.0",
        "event_id": uuid::Uuid::now_v7(),
        "spec_id": spec_id,
        "phase": "do-task",
        "agent_session_id": "session-1",
        "timestamp": chrono::Utc::now(),
        "tool": "create_task",
        "payload": payload,
    });
    let path = folder.join("phase-events.jsonl");
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string(&line).expect("json")),
    )
    .expect("write phase-events");
    path
}
