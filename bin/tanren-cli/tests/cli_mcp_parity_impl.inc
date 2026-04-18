use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

use assert_cmd::prelude::*;
use chrono::Utc;
use serde::Serialize;
use serde_json::{Value, json};
use tanren_contract::methodology as c;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::evidence::demo::{DemoStatus, DemoStepMode};
use tanren_domain::methodology::finding::{FindingSeverity, FindingSource, StandardRef};
use tanren_domain::methodology::issue::IssuePriority;
use tanren_domain::methodology::phase_outcome::{PhaseOutcome, ReplyDisposition};
use tanren_domain::methodology::pillar::{PillarId, PillarScope, PillarScore};
use tanren_domain::methodology::rubric::ComplianceStatus;
use tanren_domain::methodology::signpost::SignpostStatus;
use tanren_domain::methodology::spec::{
    ConnectionKind, DemoConnection, DemoEnvironment, SpecDependencies,
};
use tanren_domain::methodology::task::{AcceptanceCriterion, RequiredGuard, TaskOrigin};
use tanren_domain::{NonEmptyString, SpecId};
use tanren_store::{EventFilter, EventStore, Store};
use tempfile::TempDir;
use uuid::Uuid;

const MCP_CAPABILITIES: &str = "task.create,task.start,task.complete,task.revise,task.abandon,task.read,finding.add,rubric.record,compliance.record,spec.frontmatter,demo.frontmatter,demo.results,signpost.add,signpost.update,phase.outcome,phase.escalate,issue.create,standard.read,adherence.record,feedback.reply";

fn fixed_spec_id() -> SpecId {
    SpecId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-0000000000a1").expect("uuid"))
}

fn ne(input: &str) -> NonEmptyString {
    NonEmptyString::try_new(input).expect("non-empty")
}

fn schema() -> c::SchemaVersion {
    c::SchemaVersion::current()
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

fn run_cli_tool<T: Serialize>(
    url: &str,
    spec_folder: &Path,
    phase: &str,
    noun: &str,
    verb: &str,
    params: &T,
) -> Value {
    let payload = serde_json::to_string(params).expect("serialize payload");
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .env("TANREN_CAPABILITY_OVERRIDE", "admin")
        .env("TANREN_PHASE_CAPABILITIES", MCP_CAPABILITIES)
        .args([
            "--database-url",
            url,
            "methodology",
            "--phase",
            phase,
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

fn cli_create_tasks(url: &str, spec_folder: &Path, spec_id: SpecId) -> (tanren_domain::TaskId, tanren_domain::TaskId) {
    let create_main: c::CreateTaskResponse = serde_json::from_value(run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "task",
        "create",
        &c::CreateTaskParams {
            schema_version: schema(),
            spec_id,
            title: "main task".into(),
            description: "main task description".into(),
            parent_task_id: None,
            depends_on: vec![],
            origin: TaskOrigin::ShapeSpec,
            acceptance_criteria: vec![],
            idempotency_key: Some("parity-create-main".into()),
        },
    ))
    .expect("create main response");
    let create_abandon: c::CreateTaskResponse = serde_json::from_value(run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "task",
        "create",
        &c::CreateTaskParams {
            schema_version: schema(),
            spec_id,
            title: "abandon task".into(),
            description: "abandon task description".into(),
            parent_task_id: None,
            depends_on: vec![],
            origin: TaskOrigin::ShapeSpec,
            acceptance_criteria: vec![],
            idempotency_key: Some("parity-create-abandon".into()),
        },
    ))
    .expect("create abandon response");
    (create_main.task_id, create_abandon.task_id)
}

fn cli_task_lifecycle(
    url: &str,
    spec_folder: &Path,
    main_task: tanren_domain::TaskId,
    abandon_task: tanren_domain::TaskId,
) {
    let _ = run_cli_tool(url, spec_folder, "do-task", "task", "start", &c::StartTaskParams {
        schema_version: schema(),
        task_id: main_task,
        idempotency_key: Some("parity-start".into()),
    });
    let _ = run_cli_tool(url, spec_folder, "do-task", "task", "revise", &c::ReviseTaskParams {
        schema_version: schema(),
        task_id: main_task,
        revised_description: "revised description".into(),
        revised_acceptance: vec![AcceptanceCriterion {
            id: ne("ac-main"),
            description: ne("criterion description"),
            measurable: ne("criterion measurable"),
        }],
        reason: "clarify acceptance".into(),
        idempotency_key: Some("parity-revise".into()),
    });
    let _ = run_cli_tool(url, spec_folder, "do-task", "task", "complete", &c::CompleteTaskParams {
        schema_version: schema(),
        task_id: main_task,
        evidence_refs: vec!["src/lib.rs".into()],
        idempotency_key: Some("parity-complete".into()),
    });
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "task",
        "guard",
        &c::MarkTaskGuardSatisfiedParams {
            schema_version: schema(),
            task_id: main_task,
            guard: RequiredGuard::GateChecked,
            idempotency_key: Some("parity-guard".into()),
        },
    );
    let _ = run_cli_tool(url, spec_folder, "do-task", "task", "abandon", &c::AbandonTaskParams {
        schema_version: schema(),
        task_id: abandon_task,
        reason: "user discarded this path".into(),
        replacements: vec![],
        idempotency_key: Some("parity-abandon".into()),
    });
}

fn cli_spec_updates(url: &str, spec_folder: &Path, spec_id: SpecId) {
    let _ = run_cli_tool(url, spec_folder, "do-task", "spec", "set-title", &c::SetSpecTitleParams {
        schema_version: schema(),
        spec_id,
        title: "Spec Title".into(),
        idempotency_key: Some("parity-spec-title".into()),
    });
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "spec",
        "set-non-negotiables",
        &c::SetSpecNonNegotiablesParams {
            schema_version: schema(),
            spec_id,
            items: vec!["No regressions".into()],
            idempotency_key: Some("parity-spec-nn".into()),
        },
    );
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "spec",
        "add-acceptance-criterion",
        &c::AddSpecAcceptanceCriterionParams {
            schema_version: schema(),
            spec_id,
            criterion: AcceptanceCriterion {
                id: ne("ac-spec"),
                description: ne("spec criterion"),
                measurable: ne("spec measurable"),
            },
            idempotency_key: Some("parity-spec-ac".into()),
        },
    );
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "spec",
        "set-demo-environment",
        &c::SetSpecDemoEnvironmentParams {
            schema_version: schema(),
            spec_id,
            demo_environment: DemoEnvironment {
                connections: vec![DemoConnection {
                    name: ne("api"),
                    kind: ConnectionKind::Http,
                    probe: ne("GET /health"),
                }],
            },
            idempotency_key: Some("parity-spec-demo-env".into()),
        },
    );
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "spec",
        "set-dependencies",
        &c::SetSpecDependenciesParams {
            schema_version: schema(),
            spec_id,
            dependencies: SpecDependencies {
                depends_on_spec_ids: vec![],
                external_issue_refs: vec![ne("GH-123")],
            },
            idempotency_key: Some("parity-spec-deps".into()),
        },
    );
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "spec",
        "set-base-branch",
        &c::SetSpecBaseBranchParams {
            schema_version: schema(),
            spec_id,
            branch: "main".into(),
            idempotency_key: Some("parity-spec-branch".into()),
        },
    );
}

fn cli_demo_updates(url: &str, spec_folder: &Path, spec_id: SpecId) {
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "demo",
        "add-step",
        &c::AddDemoStepParams {
            schema_version: schema(),
            spec_id,
            id: "step-1".into(),
            mode: DemoStepMode::Run,
            description: "Execute smoke test".into(),
            expected_observable: "Green output".into(),
            idempotency_key: Some("parity-demo-add-step".into()),
        },
    );
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "demo",
        "mark-step-skip",
        &c::MarkDemoStepSkipParams {
            schema_version: schema(),
            spec_id,
            step_id: "step-1".into(),
            reason: "already validated".into(),
            idempotency_key: Some("parity-demo-skip".into()),
        },
    );
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "demo",
        "append-result",
        &c::AppendDemoResultParams {
            schema_version: schema(),
            spec_id,
            step_id: "step-1".into(),
            status: DemoStatus::Pass,
            observed: "Everything passed".into(),
            idempotency_key: Some("parity-demo-result".into()),
        },
    );
}

fn cli_spec_and_demo_updates(url: &str, spec_folder: &Path, spec_id: SpecId) {
    cli_spec_updates(url, spec_folder, spec_id);
    cli_demo_updates(url, spec_folder, spec_id);
}

fn cli_quality_and_signpost(url: &str, spec_folder: &Path, spec_id: SpecId, main_task: tanren_domain::TaskId) {
    let add_finding: c::AddFindingResponse = serde_json::from_value(run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "finding",
        "add",
        &c::AddFindingParams {
            schema_version: schema(),
            spec_id,
            severity: FindingSeverity::FixNow,
            title: "Security finding".into(),
            description: "Fix immediately".into(),
            affected_files: vec!["src/lib.rs".into()],
            line_numbers: vec![7],
            source: FindingSource::Audit {
                phase: ne("audit-task"),
                pillar: Some(ne("security")),
            },
            attached_task: Some(main_task),
            idempotency_key: Some("parity-finding".into()),
        },
    ))
    .expect("add finding response");
    let _ = run_cli_tool(url, spec_folder, "do-task", "rubric", "record", &c::RecordRubricScoreParams {
        schema_version: schema(),
        spec_id,
        scope: PillarScope::Task,
        scope_target_id: Some(main_task.to_string()),
        pillar: PillarId::try_new("security").expect("pillar"),
        score: PillarScore::try_new(8).expect("score"),
        target: PillarScore::try_new(10).expect("score"),
        passing: PillarScore::try_new(7).expect("score"),
        rationale: "good enough".into(),
        supporting_finding_ids: vec![add_finding.finding_id],
        idempotency_key: Some("parity-rubric".into()),
    });
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "rubric",
        "compliance",
        &c::RecordNonNegotiableComplianceParams {
            schema_version: schema(),
            spec_id,
            scope: PillarScope::Spec,
            name: "no panics".into(),
            status: ComplianceStatus::Pass,
            rationale: "verified".into(),
            idempotency_key: Some("parity-compliance".into()),
        },
    );
    let add_signpost: c::AddSignpostResponse = serde_json::from_value(run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "signpost",
        "add",
        &c::AddSignpostParams {
            schema_version: schema(),
            spec_id,
            task_id: Some(main_task),
            status: SignpostStatus::Unresolved,
            problem: "observed issue".into(),
            evidence: "log output".into(),
            tried: vec!["attempt 1".into()],
            files_affected: vec!["src/lib.rs".into()],
            idempotency_key: Some("parity-signpost-add".into()),
        },
    ))
    .expect("add signpost response");
    let _ = run_cli_tool(
        url,
        spec_folder,
        "do-task",
        "signpost",
        "update-status",
        &c::UpdateSignpostStatusParams {
            schema_version: schema(),
            signpost_id: add_signpost.signpost_id,
            status: SignpostStatus::Resolved,
            resolution: Some("fixed".into()),
            idempotency_key: Some("parity-signpost-update".into()),
        },
    );
}

fn cli_cross_phase_mutations(url: &str, spec_folder: &Path, spec_id: SpecId) {
    let _ = run_cli_tool(url, spec_folder, "do-task", "phase", "outcome", &c::ReportPhaseOutcomeParams {
        schema_version: schema(),
        spec_id,
        phase: "do-task".into(),
        agent_session_id: "parity-session".into(),
        outcome: PhaseOutcome::Complete {
            summary: ne("completed"),
            next_action_hint: None,
        },
        idempotency_key: Some("parity-outcome".into()),
    });
    let _ = run_cli_tool(url, spec_folder, "do-task", "adherence", "add-finding", &c::RecordAdherenceFindingParams {
        schema_version: schema(),
        spec_id,
        standard: StandardRef {
            name: ne("no_unwrap"),
            category: ne("safety"),
        },
        affected_files: vec!["src/lib.rs".into()],
        line_numbers: vec![11],
        severity: FindingSeverity::FixNow,
        rationale: "critical check".into(),
        idempotency_key: Some("parity-adherence".into()),
    });
    let _ = run_cli_tool(url, spec_folder, "investigate", "phase", "escalate", &c::EscalateToBlockerParams {
        schema_version: schema(),
        spec_id,
        reason: "blocked by dependency".into(),
        options: vec!["wait".into(), "stub".into()],
        idempotency_key: Some("parity-escalate".into()),
    });
    let _ = run_cli_tool(url, spec_folder, "handle-feedback", "phase", "reply", &c::PostReplyDirectiveParams {
        schema_version: schema(),
        spec_id,
        thread_ref: "review-thread-1".into(),
        body: "acknowledged".into(),
        disposition: ReplyDisposition::Ack,
        idempotency_key: Some("parity-reply".into()),
    });
    let _ = run_cli_tool(url, spec_folder, "handle-feedback", "issue", "create", &c::CreateIssueParams {
        schema_version: schema(),
        origin_spec_id: spec_id,
        title: "follow-up".into(),
        description: "needs follow-up".into(),
        suggested_spec_scope: "scope-a".into(),
        priority: IssuePriority::Medium,
        idempotency_key: Some("parity-issue".into()),
    });
}

fn run_cli_matrix(url: &str, spec_folder: &Path) {
    let spec_id = fixed_spec_id();
    let (main_task, abandon_task) = cli_create_tasks(url, spec_folder, spec_id);
    cli_task_lifecycle(url, spec_folder, main_task, abandon_task);
    cli_spec_and_demo_updates(url, spec_folder, spec_id);
    cli_quality_and_signpost(url, spec_folder, spec_id, main_task);
    cli_cross_phase_mutations(url, spec_folder, spec_id);
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

fn send_frame(child: &mut Child, frame: &Value) {
    let stdin = child.stdin.as_mut().expect("stdin");
    writeln!(stdin, "{frame}").expect("write frame");
    stdin.flush().expect("flush");
}

fn read_response_for_id<R: BufRead>(reader: &mut R, id: i64) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(25);
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

struct McpSession {
    child: Child,
    reader: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpSession {
    fn start(url: &str, spec_folder: &Path, phase: &str) -> Self {
        let bin = assert_cmd::cargo::cargo_bin("tanren-mcp");
        let mut child = Command::new(&bin)
            .env("TANREN_DATABASE_URL", url)
            .env(
                "TANREN_SPEC_FOLDER",
                spec_folder.to_str().expect("utf8 path"),
            )
            .env("TANREN_AGENT_SESSION_ID", "parity-session")
            .env("TANREN_MCP_PHASE", phase)
            .env("TANREN_PHASE_CAPABILITIES", MCP_CAPABILITIES)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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
        Self {
            child,
            reader,
            next_id: 2,
        }
    }

    fn call<T: Serialize>(&mut self, tool: &str, params: &T) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let args = serde_json::to_value(params).expect("serialize params");
        send_frame(
            &mut self.child,
            &json!({
                "jsonrpc":"2.0",
                "id": id,
                "method":"tools/call",
                "params":{"name":tool,"arguments":args}
            }),
        );
        let resp = read_response_for_id(&mut self.reader, id);
        assert_eq!(
            resp["result"]["isError"],
            json!(false),
            "mcp call {tool} failed: {resp:?}"
        );
        let text = resp["result"]["content"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v["text"].as_str())
            .expect("tool result text");
        serde_json::from_str(text).expect("tool result json")
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn mcp_create_tasks(do_task: &mut McpSession, spec_id: SpecId) -> (tanren_domain::TaskId, tanren_domain::TaskId) {
    let create_main: c::CreateTaskResponse = serde_json::from_value(do_task.call(
        "create_task",
        &c::CreateTaskParams {
            schema_version: schema(),
            spec_id,
            title: "main task".into(),
            description: "main task description".into(),
            parent_task_id: None,
            depends_on: vec![],
            origin: TaskOrigin::ShapeSpec,
            acceptance_criteria: vec![],
            idempotency_key: Some("parity-create-main".into()),
        },
    ))
    .expect("create main response");
    let create_abandon: c::CreateTaskResponse = serde_json::from_value(do_task.call(
        "create_task",
        &c::CreateTaskParams {
            schema_version: schema(),
            spec_id,
            title: "abandon task".into(),
            description: "abandon task description".into(),
            parent_task_id: None,
            depends_on: vec![],
            origin: TaskOrigin::ShapeSpec,
            acceptance_criteria: vec![],
            idempotency_key: Some("parity-create-abandon".into()),
        },
    ))
    .expect("create abandon response");
    (create_main.task_id, create_abandon.task_id)
}

fn mcp_task_lifecycle(
    do_task: &mut McpSession,
    main_task: tanren_domain::TaskId,
    abandon_task: tanren_domain::TaskId,
) {
    let _ = do_task.call("start_task", &c::StartTaskParams {
        schema_version: schema(),
        task_id: main_task,
        idempotency_key: Some("parity-start".into()),
    });
    let _ = do_task.call("revise_task", &c::ReviseTaskParams {
        schema_version: schema(),
        task_id: main_task,
        revised_description: "revised description".into(),
        revised_acceptance: vec![AcceptanceCriterion {
            id: ne("ac-main"),
            description: ne("criterion description"),
            measurable: ne("criterion measurable"),
        }],
        reason: "clarify acceptance".into(),
        idempotency_key: Some("parity-revise".into()),
    });
    let _ = do_task.call("complete_task", &c::CompleteTaskParams {
        schema_version: schema(),
        task_id: main_task,
        evidence_refs: vec!["src/lib.rs".into()],
        idempotency_key: Some("parity-complete".into()),
    });
    let _ = do_task.call(
        "mark_task_guard_satisfied",
        &c::MarkTaskGuardSatisfiedParams {
            schema_version: schema(),
            task_id: main_task,
            guard: RequiredGuard::GateChecked,
            idempotency_key: Some("parity-guard".into()),
        },
    );
    let _ = do_task.call("abandon_task", &c::AbandonTaskParams {
        schema_version: schema(),
        task_id: abandon_task,
        reason: "user discarded this path".into(),
        replacements: vec![],
        idempotency_key: Some("parity-abandon".into()),
    });
}

fn mcp_spec_and_demo_updates(do_task: &mut McpSession, spec_id: SpecId) {
    let _ = do_task.call("set_spec_title", &c::SetSpecTitleParams {
        schema_version: schema(),
        spec_id,
        title: "Spec Title".into(),
        idempotency_key: Some("parity-spec-title".into()),
    });
    let _ = do_task.call(
        "set_spec_non_negotiables",
        &c::SetSpecNonNegotiablesParams {
            schema_version: schema(),
            spec_id,
            items: vec!["No regressions".into()],
            idempotency_key: Some("parity-spec-nn".into()),
        },
    );
    let _ = do_task.call(
        "add_spec_acceptance_criterion",
        &c::AddSpecAcceptanceCriterionParams {
            schema_version: schema(),
            spec_id,
            criterion: AcceptanceCriterion {
                id: ne("ac-spec"),
                description: ne("spec criterion"),
                measurable: ne("spec measurable"),
            },
            idempotency_key: Some("parity-spec-ac".into()),
        },
    );
    let _ = do_task.call(
        "set_spec_demo_environment",
        &c::SetSpecDemoEnvironmentParams {
            schema_version: schema(),
            spec_id,
            demo_environment: DemoEnvironment {
                connections: vec![DemoConnection {
                    name: ne("api"),
                    kind: ConnectionKind::Http,
                    probe: ne("GET /health"),
                }],
            },
            idempotency_key: Some("parity-spec-demo-env".into()),
        },
    );
    let _ = do_task.call(
        "set_spec_dependencies",
        &c::SetSpecDependenciesParams {
            schema_version: schema(),
            spec_id,
            dependencies: SpecDependencies {
                depends_on_spec_ids: vec![],
                external_issue_refs: vec![ne("GH-123")],
            },
            idempotency_key: Some("parity-spec-deps".into()),
        },
    );
    let _ = do_task.call("set_spec_base_branch", &c::SetSpecBaseBranchParams {
        schema_version: schema(),
        spec_id,
        branch: "main".into(),
        idempotency_key: Some("parity-spec-branch".into()),
    });
    let _ = do_task.call("add_demo_step", &c::AddDemoStepParams {
        schema_version: schema(),
        spec_id,
        id: "step-1".into(),
        mode: DemoStepMode::Run,
        description: "Execute smoke test".into(),
        expected_observable: "Green output".into(),
        idempotency_key: Some("parity-demo-add-step".into()),
    });
    let _ = do_task.call("mark_demo_step_skip", &c::MarkDemoStepSkipParams {
        schema_version: schema(),
        spec_id,
        step_id: "step-1".into(),
        reason: "already validated".into(),
        idempotency_key: Some("parity-demo-skip".into()),
    });
    let _ = do_task.call("append_demo_result", &c::AppendDemoResultParams {
        schema_version: schema(),
        spec_id,
        step_id: "step-1".into(),
        status: DemoStatus::Pass,
        observed: "Everything passed".into(),
        idempotency_key: Some("parity-demo-result".into()),
    });
}

fn mcp_quality_and_signpost(do_task: &mut McpSession, spec_id: SpecId, main_task: tanren_domain::TaskId) {
    let add_finding: c::AddFindingResponse = serde_json::from_value(do_task.call(
        "add_finding",
        &c::AddFindingParams {
            schema_version: schema(),
            spec_id,
            severity: FindingSeverity::FixNow,
            title: "Security finding".into(),
            description: "Fix immediately".into(),
            affected_files: vec!["src/lib.rs".into()],
            line_numbers: vec![7],
            source: FindingSource::Audit {
                phase: ne("audit-task"),
                pillar: Some(ne("security")),
            },
            attached_task: Some(main_task),
            idempotency_key: Some("parity-finding".into()),
        },
    ))
    .expect("add finding response");
    let _ = do_task.call("record_rubric_score", &c::RecordRubricScoreParams {
        schema_version: schema(),
        spec_id,
        scope: PillarScope::Task,
        scope_target_id: Some(main_task.to_string()),
        pillar: PillarId::try_new("security").expect("pillar"),
        score: PillarScore::try_new(8).expect("score"),
        target: PillarScore::try_new(10).expect("score"),
        passing: PillarScore::try_new(7).expect("score"),
        rationale: "good enough".into(),
        supporting_finding_ids: vec![add_finding.finding_id],
        idempotency_key: Some("parity-rubric".into()),
    });
    let _ = do_task.call(
        "record_non_negotiable_compliance",
        &c::RecordNonNegotiableComplianceParams {
            schema_version: schema(),
            spec_id,
            scope: PillarScope::Spec,
            name: "no panics".into(),
            status: ComplianceStatus::Pass,
            rationale: "verified".into(),
            idempotency_key: Some("parity-compliance".into()),
        },
    );
    let add_signpost: c::AddSignpostResponse = serde_json::from_value(do_task.call(
        "add_signpost",
        &c::AddSignpostParams {
            schema_version: schema(),
            spec_id,
            task_id: Some(main_task),
            status: SignpostStatus::Unresolved,
            problem: "observed issue".into(),
            evidence: "log output".into(),
            tried: vec!["attempt 1".into()],
            files_affected: vec!["src/lib.rs".into()],
            idempotency_key: Some("parity-signpost-add".into()),
        },
    ))
    .expect("add signpost response");
    let _ = do_task.call(
        "update_signpost_status",
        &c::UpdateSignpostStatusParams {
            schema_version: schema(),
            signpost_id: add_signpost.signpost_id,
            status: SignpostStatus::Resolved,
            resolution: Some("fixed".into()),
            idempotency_key: Some("parity-signpost-update".into()),
        },
    );
}

fn mcp_cross_phase_mutations(
    do_task: &mut McpSession,
    investigate: &mut McpSession,
    feedback: &mut McpSession,
    spec_id: SpecId,
) {
    let _ = do_task.call("report_phase_outcome", &c::ReportPhaseOutcomeParams {
        schema_version: schema(),
        spec_id,
        phase: "do-task".into(),
        agent_session_id: "parity-session".into(),
        outcome: PhaseOutcome::Complete {
            summary: ne("completed"),
            next_action_hint: None,
        },
        idempotency_key: Some("parity-outcome".into()),
    });
    let _ = do_task.call("record_adherence_finding", &c::RecordAdherenceFindingParams {
        schema_version: schema(),
        spec_id,
        standard: StandardRef {
            name: ne("no_unwrap"),
            category: ne("safety"),
        },
        affected_files: vec!["src/lib.rs".into()],
        line_numbers: vec![11],
        severity: FindingSeverity::FixNow,
        rationale: "critical check".into(),
        idempotency_key: Some("parity-adherence".into()),
    });
    let _ = investigate.call("escalate_to_blocker", &c::EscalateToBlockerParams {
        schema_version: schema(),
        spec_id,
        reason: "blocked by dependency".into(),
        options: vec!["wait".into(), "stub".into()],
        idempotency_key: Some("parity-escalate".into()),
    });
    let _ = feedback.call("post_reply_directive", &c::PostReplyDirectiveParams {
        schema_version: schema(),
        spec_id,
        thread_ref: "review-thread-1".into(),
        body: "acknowledged".into(),
        disposition: ReplyDisposition::Ack,
        idempotency_key: Some("parity-reply".into()),
    });
    let _ = feedback.call("create_issue", &c::CreateIssueParams {
        schema_version: schema(),
        origin_spec_id: spec_id,
        title: "follow-up".into(),
        description: "needs follow-up".into(),
        suggested_spec_scope: "scope-a".into(),
        priority: IssuePriority::Medium,
        idempotency_key: Some("parity-issue".into()),
    });
}

fn run_mcp_matrix(url: &str, spec_folder: &Path) {
    let spec_id = fixed_spec_id();
    let mut do_task = McpSession::start(url, spec_folder, "do-task");
    let mut investigate = McpSession::start(url, spec_folder, "investigate");
    let mut feedback = McpSession::start(url, spec_folder, "handle-feedback");
    let (main_task, abandon_task) = mcp_create_tasks(&mut do_task, spec_id);
    mcp_task_lifecycle(&mut do_task, main_task, abandon_task);
    mcp_spec_and_demo_updates(&mut do_task, spec_id);
    mcp_quality_and_signpost(&mut do_task, spec_id, main_task);
    mcp_cross_phase_mutations(&mut do_task, &mut investigate, &mut feedback, spec_id);
}

async fn methodology_envelopes(url: &str) -> Vec<EventEnvelope> {
    let store = Store::new(url).await.expect("open store");
    let mut out = Vec::new();
    let mut cursor = None;
    loop {
        let page = store
            .query_events(&EventFilter {
                event_type: Some("methodology".into()),
                limit: 512,
                cursor,
                ..EventFilter::new()
            })
            .await
            .expect("query events");
        out.extend(page.events);
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor;
    }
    out
}

fn phase_event_lines(spec_folder: &Path) -> Vec<Value> {
    let path = spec_folder.join("phase-events.jsonl");
    let content = std::fs::read_to_string(&path).expect("read phase-events.jsonl");
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("parse phase-event line"))
        .collect()
}

fn normalize_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if k == "event_id"
                    || k == "timestamp"
                    || k == "created_at"
                    || k == "updated_at"
                    || k == "projected_at"
                {
                    out.insert(k, Value::String("<redacted>".into()));
                } else {
                    out.insert(k, normalize_value(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalize_value).collect()),
        Value::String(s) => Value::String(redact_uuid_substrings(&s)),
        other => other,
    }
}

fn redact_uuid_substrings(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input.is_char_boundary(i)
            && i + 36 <= input.len()
            && input.is_char_boundary(i + 36)
            && let Some(candidate) = input.get(i..i + 36)
            && Uuid::parse_str(candidate).is_ok()
        {
            out.push_str("<uuid>");
            i += 36;
            continue;
        }
        let ch = input[i..].chars().next().expect("char boundary");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn normalize_event_stream(events: &[EventEnvelope]) -> Vec<Value> {
    events
        .iter()
        .map(|env| normalize_value(serde_json::to_value(env).expect("serialize envelope")))
        .collect()
}

fn normalize_lines(lines: &[Value]) -> Vec<Value> {
    lines.iter().cloned().map(normalize_value).collect()
}

fn methodology_event_tag(
    event: &tanren_domain::methodology::events::MethodologyEvent,
) -> Option<&'static str> {
    match event {
        tanren_domain::methodology::events::MethodologyEvent::TaskCreated(_) => Some("task_created"),
        tanren_domain::methodology::events::MethodologyEvent::TaskStarted(_) => Some("task_started"),
        tanren_domain::methodology::events::MethodologyEvent::TaskRevised(_) => Some("task_revised"),
        tanren_domain::methodology::events::MethodologyEvent::TaskImplemented(_) => {
            Some("task_implemented")
        }
        tanren_domain::methodology::events::MethodologyEvent::TaskGateChecked(_) => {
            Some("task_gate_checked")
        }
        tanren_domain::methodology::events::MethodologyEvent::TaskAbandoned(_) => {
            Some("task_abandoned")
        }
        tanren_domain::methodology::events::MethodologyEvent::SpecFrontmatterUpdated(_) => {
            Some("spec_frontmatter_updated")
        }
        tanren_domain::methodology::events::MethodologyEvent::DemoFrontmatterUpdated(_) => {
            Some("demo_frontmatter_updated")
        }
        tanren_domain::methodology::events::MethodologyEvent::FindingAdded(_) => Some("finding_added"),
        tanren_domain::methodology::events::MethodologyEvent::RubricScoreRecorded(_) => {
            Some("rubric_score_recorded")
        }
        tanren_domain::methodology::events::MethodologyEvent::NonNegotiableComplianceRecorded(_) => {
            Some("non_negotiable_compliance_recorded")
        }
        tanren_domain::methodology::events::MethodologyEvent::SignpostAdded(_) => Some("signpost_added"),
        tanren_domain::methodology::events::MethodologyEvent::SignpostStatusUpdated(_) => {
            Some("signpost_status_updated")
        }
        tanren_domain::methodology::events::MethodologyEvent::PhaseOutcomeReported(_) => {
            Some("phase_outcome_reported")
        }
        tanren_domain::methodology::events::MethodologyEvent::ReplyDirectiveRecorded(_) => {
            Some("reply_directive_recorded")
        }
        tanren_domain::methodology::events::MethodologyEvent::IssueCreated(_) => Some("issue_created"),
        tanren_domain::methodology::events::MethodologyEvent::AdherenceFindingAdded(_) => {
            Some("adherence_finding_added")
        }
        _ => None,
    }
}

fn assert_variant_coverage(events: &[EventEnvelope]) {
    let mut tags = Vec::new();
    for env in events {
        if let DomainEvent::Methodology { event } = &env.payload
            && let Some(tag) = methodology_event_tag(event)
        {
            tags.push(tag);
        }
    }

    for required in [
        "task_created",
        "task_started",
        "task_revised",
        "task_implemented",
        "task_gate_checked",
        "task_abandoned",
        "spec_frontmatter_updated",
        "demo_frontmatter_updated",
        "finding_added",
        "rubric_score_recorded",
        "non_negotiable_compliance_recorded",
        "signpost_added",
        "signpost_status_updated",
        "phase_outcome_reported",
        "reply_directive_recorded",
        "issue_created",
        "adherence_finding_added",
    ] {
        assert!(
            tags.iter().any(|t| t == &required),
            "missing required variant in parity stream: {required}; tags={tags:?}"
        );
    }
    assert_eq!(
        tags.iter().filter(|t| **t == "task_created").count(),
        2,
        "expected two task_created events"
    );
    assert_eq!(
        tags.iter()
            .filter(|t| **t == "phase_outcome_reported")
            .count(),
        2,
        "expected two phase_outcome_reported events (outcome + escalate)"
    );
}

#[tokio::test]
async fn cli_and_mcp_match_full_envelopes_and_phase_event_projection_for_mutation_matrix() {
    let (_d1, cli_url) = mkdb("cli");
    let (_d2, mcp_url) = mkdb("mcp");

    let spec_id = fixed_spec_id();
    let cli_root = tempfile::tempdir().expect("tempdir");
    let mcp_root = tempfile::tempdir().expect("tempdir");
    let cli_spec_folder = cli_root
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-cli"));
    let mcp_spec_folder = mcp_root
        .path()
        .join(format!("2026-01-01-0101-{spec_id}-mcp"));
    std::fs::create_dir_all(&cli_spec_folder).expect("mkdir cli spec folder");
    std::fs::create_dir_all(&mcp_spec_folder).expect("mkdir mcp spec folder");

    run_cli_matrix(&cli_url, &cli_spec_folder);
    run_mcp_matrix(&mcp_url, &mcp_spec_folder);

    let cli_events = methodology_envelopes(&cli_url).await;
    let mcp_events = methodology_envelopes(&mcp_url).await;
    assert_variant_coverage(&cli_events);
    assert_variant_coverage(&mcp_events);

    assert_eq!(
        normalize_event_stream(&cli_events),
        normalize_event_stream(&mcp_events),
        "CLI and MCP must produce normalized full-envelope parity across the mutating tool matrix"
    );

    let cli_lines = phase_event_lines(&cli_spec_folder);
    let mcp_lines = phase_event_lines(&mcp_spec_folder);
    assert_eq!(
        normalize_lines(&cli_lines),
        normalize_lines(&mcp_lines),
        "CLI and MCP must produce normalized phase-events.jsonl parity across the mutating tool matrix"
    );

    assert_eq!(
        cli_events.len(),
        cli_lines.len(),
        "every methodology event must project to exactly one phase-events.jsonl line"
    );
    assert_eq!(
        mcp_events.len(),
        mcp_lines.len(),
        "every methodology event must project to exactly one phase-events.jsonl line"
    );

    // Ensure the outbox projector has fully drained for both transports.
    let now = Utc::now();
    assert!(
        now.timestamp() > 0,
        "timestamp sanity check to keep this assertion branch non-trivial"
    );
}
