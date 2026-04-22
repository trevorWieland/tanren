use super::*;

use chrono::TimeZone as _;
use sha2::{Digest as _, Sha256};
use tanren_domain::TaskId;
use tanren_domain::methodology::event_tool::PhaseEventOriginKind;
use tanren_domain::methodology::events::{
    MethodologyEvent, SpecDefined, TaskAdherent, TaskAudited, TaskCompleted, TaskCreated,
    TaskGateChecked, TaskImplemented, TaskStarted,
};
use tanren_domain::methodology::spec::{
    DemoEnvironment, Spec, SpecDependencies, SpecRelevanceContext,
};
use tanren_domain::methodology::task::{AcceptanceCriterion, TaskOrigin, TaskStatus};
use uuid::Uuid;

fn ne(value: &str) -> NonEmptyString {
    NonEmptyString::try_new(value.to_owned()).expect("non-empty")
}

fn ts(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .expect("valid timestamp")
}

fn spec_id() -> SpecId {
    SpecId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-0000000008a1").expect("uuid"))
}

fn task_id() -> TaskId {
    TaskId::from_uuid(Uuid::parse_str("019da764-e015-7af1-9385-8a7b98995e90").expect("uuid"))
}

fn event_id(s: &str) -> EventId {
    EventId::from_uuid(Uuid::parse_str(s).expect("uuid"))
}

fn mk_spec(spec_id: SpecId, created_at: DateTime<Utc>) -> Spec {
    Spec {
        id: spec_id,
        title: ne("Typed Artifact Spec"),
        problem_statement: Some(ne("Stabilize event-projected artifacts.")),
        motivations: vec![ne("Eliminate manual drift.")],
        expectations: vec![ne("Artifacts regenerate deterministically.")],
        planned_behaviors: vec![ne("shape-spec emits typed events for spec sections.")],
        implementation_plan: vec![
            ne("Build projector."),
            ne("Wire finalize/reconcile."),
            ne("Validate with full-suite CI."),
        ],
        non_negotiables: vec![ne("No manual edits to orchestrator-owned artifacts.")],
        acceptance_criteria: vec![AcceptanceCriterion {
            id: ne("ac-1"),
            description: ne("plan.md always generated"),
            measurable: ne("present after every mutating call"),
        }],
        demo_environment: DemoEnvironment::default(),
        dependencies: SpecDependencies::default(),
        base_branch: ne("main"),
        touched_symbols: vec![],
        relevance_context: SpecRelevanceContext::default(),
        created_at,
    }
}

fn mk_task(spec_id: SpecId, created_at: DateTime<Utc>) -> Task {
    Task {
        id: task_id(),
        spec_id,
        title: ne("Create projector"),
        description: "Build typed projector and renderers".to_owned(),
        acceptance_criteria: vec![AcceptanceCriterion {
            id: ne("task-ac-1"),
            description: ne("Artifacts are generated"),
            measurable: ne("spec.md/plan.md/tasks.* exist"),
        }],
        origin: TaskOrigin::ShapeSpec,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at,
        updated_at: created_at,
    }
}

fn mk_line(
    event_id: EventId,
    spec_id: SpecId,
    timestamp: DateTime<Utc>,
    phase: &str,
    tool: &str,
    payload: MethodologyEvent,
) -> PhaseEventLine {
    PhaseEventLine {
        schema_version: PHASE_EVENT_LINE_SCHEMA_VERSION.to_owned(),
        event_id,
        spec_id,
        phase: phase.to_owned(),
        agent_session_id: "session-a".to_owned(),
        timestamp,
        caused_by_tool_call_id: None,
        origin_kind: PhaseEventOriginKind::ToolPrimary,
        tool: tool.to_owned(),
        payload,
    }
}

fn sample_lines() -> Vec<PhaseEventLine> {
    let spec_id = spec_id();
    let t0 = ts(2026, 4, 21, 12, 0, 0);
    let t1 = ts(2026, 4, 21, 12, 1, 0);
    let t2 = ts(2026, 4, 21, 12, 2, 0);
    let t3 = ts(2026, 4, 21, 12, 3, 0);
    let t4 = ts(2026, 4, 21, 12, 4, 0);
    let t5 = ts(2026, 4, 21, 12, 5, 0);
    let t6 = ts(2026, 4, 21, 12, 6, 0);
    let t7 = ts(2026, 4, 21, 12, 7, 0);
    let mut lines = sample_setup_lines(spec_id, t0, t1);
    lines.extend(sample_task_transition_lines(
        spec_id,
        [t2, t3, t4, t5, t6, t7],
    ));
    lines
}

fn sample_setup_lines(
    spec_id: SpecId,
    spec_defined_at: DateTime<Utc>,
    task_created_at: DateTime<Utc>,
) -> Vec<PhaseEventLine> {
    vec![
        mk_line(
            event_id("019da764-e015-7af1-9385-8a7b98995e01"),
            spec_id,
            spec_defined_at,
            "shape-spec",
            "shape-spec",
            MethodologyEvent::SpecDefined(SpecDefined {
                spec: Box::new(mk_spec(spec_id, spec_defined_at)),
            }),
        ),
        mk_line(
            event_id("019da764-e015-7af1-9385-8a7b98995e02"),
            spec_id,
            task_created_at,
            "shape-spec",
            "create_task",
            MethodologyEvent::TaskCreated(TaskCreated {
                task: Box::new(mk_task(spec_id, task_created_at)),
                origin: TaskOrigin::ShapeSpec,
                idempotency_key: None,
            }),
        ),
    ]
}

fn sample_task_transition_lines(
    spec_id: SpecId,
    timestamps: [DateTime<Utc>; 6],
) -> Vec<PhaseEventLine> {
    let [
        started_at,
        implemented_at,
        gate_at,
        audited_at,
        adherent_at,
        completed_at,
    ] = timestamps;
    vec![
        mk_line(
            event_id("019da764-e015-7af1-9385-8a7b98995e03"),
            spec_id,
            started_at,
            "do-task",
            "start_task",
            MethodologyEvent::TaskStarted(TaskStarted {
                task_id: task_id(),
                spec_id,
            }),
        ),
        mk_line(
            event_id("019da764-e015-7af1-9385-8a7b98995e04"),
            spec_id,
            implemented_at,
            "do-task",
            "complete_task",
            MethodologyEvent::TaskImplemented(TaskImplemented {
                task_id: task_id(),
                spec_id,
                evidence_refs: vec!["proof://event/implemented".to_owned()],
            }),
        ),
        mk_line(
            event_id("019da764-e015-7af1-9385-8a7b98995e05"),
            spec_id,
            gate_at,
            "task-gate",
            "mark_task_guard_satisfied",
            MethodologyEvent::TaskGateChecked(TaskGateChecked {
                task_id: task_id(),
                spec_id,
                idempotency_key: None,
            }),
        ),
        mk_line(
            event_id("019da764-e015-7af1-9385-8a7b98995e06"),
            spec_id,
            audited_at,
            "audit-task",
            "mark_task_guard_satisfied",
            MethodologyEvent::TaskAudited(TaskAudited {
                task_id: task_id(),
                spec_id,
                idempotency_key: None,
            }),
        ),
        mk_line(
            event_id("019da764-e015-7af1-9385-8a7b98995e07"),
            spec_id,
            adherent_at,
            "adhere-task",
            "mark_task_guard_satisfied",
            MethodologyEvent::TaskAdherent(TaskAdherent {
                task_id: task_id(),
                spec_id,
                idempotency_key: None,
            }),
        ),
        mk_line(
            event_id("019da764-e015-7af1-9385-8a7b98995e08"),
            spec_id,
            completed_at,
            "do-task",
            "mark_task_guard_satisfied",
            MethodologyEvent::TaskCompleted(TaskCompleted {
                task_id: task_id(),
                spec_id,
            }),
        ),
    ]
}

#[test]
fn render_from_lines_projects_status_and_event_rationale() {
    let lines = sample_lines();
    let rendered = render_from_lines(
        spec_id(),
        &lines,
        &[
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ],
    )
    .expect("render");

    assert!(rendered.plan_md.contains("## Ordered Implementation Plan"));
    assert!(rendered.plan_md.contains("completion guards converged"));
    assert!(
        rendered
            .plan_md
            .contains("019da764-e015-7af1-9385-8a7b98995e08")
    );
    assert!(
        rendered
            .tasks_md
            .contains("| Task ID | Title | Owner/Phase | Status |")
    );

    let tasks: serde_json::Value =
        serde_json::from_str(&rendered.tasks_json).expect("tasks json parse");
    assert_eq!(
        tasks["tasks"][0]["task"]["status"]["state"].as_str(),
        Some("complete")
    );
    assert_eq!(
        tasks["tasks"][0]["status_event_id"].as_str(),
        Some("019da764-e015-7af1-9385-8a7b98995e08")
    );

    let progress: serde_json::Value =
        serde_json::from_str(&rendered.progress_json).expect("progress json parse");
    assert_eq!(progress["task_counts"]["total"].as_u64(), Some(1));
    assert_eq!(progress["task_counts"]["complete"].as_u64(), Some(1));
}

#[test]
fn render_from_lines_is_deterministic_for_same_event_stream() {
    let lines = sample_lines();
    let a = render_from_lines(spec_id(), &lines, &[]).expect("first render");
    let b = render_from_lines(spec_id(), &lines, &[]).expect("second render");
    assert_eq!(a.spec_md, b.spec_md);
    assert_eq!(a.plan_md, b.plan_md);
    assert_eq!(a.tasks_md, b.tasks_md);
    assert_eq!(a.tasks_json, b.tasks_json);
    assert_eq!(a.demo_md, b.demo_md);
    assert_eq!(a.progress_json, b.progress_json);
    assert_eq!(a.manifest_json, b.manifest_json);
}

#[test]
fn read_phase_event_lines_rejects_schema_version_mismatch() {
    let root = tempfile::tempdir().expect("tempdir");
    let file = root.path().join("phase-events.jsonl");
    let mut line = sample_lines().remove(0);
    line.schema_version = "0.9.0".to_owned();
    std::fs::write(
        &file,
        format!("{}\n", serde_json::to_string(&line).expect("line json")),
    )
    .expect("write");

    let err = read_phase_event_lines(&file).expect_err("schema mismatch must fail");
    assert!(matches!(err, MethodologyError::FieldValidation { .. }));
    let message = err.to_string();
    assert!(message.contains("/schema_version"));
    assert!(message.contains("1.0.0"));
}

fn sha256_hex(input: &str) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(input.as_bytes());
    digest.iter().fold(
        String::with_capacity(digest.len().saturating_mul(2)),
        |mut acc, byte| {
            let _ = write!(&mut acc, "{byte:02x}");
            acc
        },
    )
}

#[test]
fn projected_markdown_full_document_snapshots_are_stable() {
    let lines = sample_lines();
    let rendered = render_from_lines(
        spec_id(),
        &lines,
        &[
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ],
    )
    .expect("render");

    let snapshots = [
        (
            "spec.md",
            sha256_hex(&rendered.spec_md),
            "5c386a6e127642e97d1a88dd5a57cc915e6cc5a0d850bea366aa5c4823808ea8",
        ),
        (
            "plan.md",
            sha256_hex(&rendered.plan_md),
            "91deed9a9c72910b42fd6f896665664c418b88e8ee204f5af85d7baf97d1b0f1",
        ),
        (
            "tasks.md",
            sha256_hex(&rendered.tasks_md),
            "98b02d2e27105fc9ecf80a4962a914d38b3962fbde2a172cd8e33b798f24d17f",
        ),
        (
            "demo.md",
            sha256_hex(&rendered.demo_md),
            "a929e3b22d01da44798c609fd96d55757eaf5a9ec4f9d7b4f72b18bc531dc7c3",
        ),
        (
            "audit.md",
            sha256_hex(&rendered.audit_md),
            "4be59ca62063bbe6401836ee4dc8cacfc22b947f0b92942acaf6496e4e079871",
        ),
        (
            "signposts.md",
            sha256_hex(&rendered.signposts_md),
            "4a45f46e3c987f8238cf12db3a11675ffd66ad8158cb1cce46aeec6ed3538cfe",
        ),
    ];

    let mut mismatches = Vec::new();
    for (artifact, actual, expected) in snapshots {
        if actual != expected {
            mismatches.push(format!("{artifact}={actual}"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "snapshot mismatches: {}",
        mismatches.join(", ")
    );
}

#[path = "artifact_projection_tests_checkpoint.rs"]
mod checkpoint_tests;
