use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tanren_app_services::methodology::renderer::render_command;
use tanren_app_services::methodology::{
    CapabilityScope, MethodologyService, config::MethodologyProfile,
};
use tanren_contract::methodology::{AddFindingParams, CreateTaskParams, RecordRubricScoreParams};
use tanren_domain::SpecId;
use tanren_domain::methodology::finding::{FindingSeverity, FindingSource};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::pillar::{PillarId, PillarScope, PillarScore};
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin};
use tanren_store::Store;

async fn mk_service(required: Vec<RequiredGuard>) -> MethodologyService {
    let store = Store::open_and_migrate("sqlite::memory:?cache=shared")
        .await
        .expect("open");
    let runtime = tanren_app_services::methodology::service::PhaseEventsRuntime {
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-methodology-rubric-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "test-session".into(),
    };
    MethodologyService::with_runtime(Arc::new(store), required, Some(runtime), vec![])
}

fn admin_scope() -> CapabilityScope {
    use tanren_domain::methodology::capability::ToolCapability::{
        FindingAdd, RubricRecord, TaskCreate,
    };
    CapabilityScope::from_iter_caps([TaskCreate, FindingAdd, RubricRecord])
}

fn phase(tag: &str) -> PhaseId {
    PhaseId::try_new(tag).expect("phase")
}

#[tokio::test]
async fn rubric_score_rejects_supporting_finding_with_mismatched_pillar() {
    let svc = mk_service(vec![]).await;
    let scope = admin_scope();
    let spec_id = SpecId::new();
    let create = svc
        .create_task(
            &scope,
            &phase("audit-task"),
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id,
                title: "T".into(),
                description: String::new(),
                acceptance_criteria: vec![],
                depends_on: vec![],
                parent_task_id: None,
                origin: TaskOrigin::ShapeSpec,
            },
        )
        .await
        .expect("create");
    let finding = svc
        .add_finding(
            &scope,
            &phase("audit-task"),
            AddFindingParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                severity: FindingSeverity::FixNow,
                title: "mismatch".into(),
                description: String::new(),
                affected_files: vec![],
                line_numbers: vec![],
                source: FindingSource::Audit {
                    phase: PhaseId::try_new("audit-task").expect("phase"),
                    pillar: Some(
                        tanren_domain::NonEmptyString::try_new("performance").expect("pillar"),
                    ),
                },
                attached_task: Some(create.task_id),
                idempotency_key: None,
            },
        )
        .await
        .expect("finding");
    let err = svc
        .record_rubric_score(
            &scope,
            &phase("audit-task"),
            RecordRubricScoreParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                scope: PillarScope::Task,
                scope_target_id: Some(create.task_id.to_string()),
                pillar: PillarId::try_new("security").expect("pillar"),
                score: PillarScore::try_new(6).expect("score"),
                target: PillarScore::try_new(10).expect("target"),
                passing: PillarScore::try_new(7).expect("passing"),
                rationale: "reason".into(),
                supporting_finding_ids: vec![finding.finding_id],
                idempotency_key: None,
            },
        )
        .await
        .expect_err("must reject mismatched pillar");
    assert!(
        err.to_string().contains("/supporting_finding_ids/0"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn rubric_score_rejects_non_actionable_supporting_severity() {
    let svc = mk_service(vec![]).await;
    let scope = admin_scope();
    let spec_id = SpecId::new();
    let create = svc
        .create_task(
            &scope,
            &phase("audit-task"),
            CreateTaskParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                idempotency_key: None,
                spec_id,
                title: "T".into(),
                description: String::new(),
                acceptance_criteria: vec![],
                depends_on: vec![],
                parent_task_id: None,
                origin: TaskOrigin::ShapeSpec,
            },
        )
        .await
        .expect("create");
    let finding = svc
        .add_finding(
            &scope,
            &phase("audit-task"),
            AddFindingParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                severity: FindingSeverity::Note,
                title: "note".into(),
                description: String::new(),
                affected_files: vec![],
                line_numbers: vec![],
                source: FindingSource::Audit {
                    phase: PhaseId::try_new("audit-task").expect("phase"),
                    pillar: Some(
                        tanren_domain::NonEmptyString::try_new("security").expect("pillar"),
                    ),
                },
                attached_task: Some(create.task_id),
                idempotency_key: None,
            },
        )
        .await
        .expect("finding");
    let err = svc
        .record_rubric_score(
            &scope,
            &phase("audit-task"),
            RecordRubricScoreParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                scope: PillarScope::Task,
                scope_target_id: Some(create.task_id.to_string()),
                pillar: PillarId::try_new("security").expect("pillar"),
                score: PillarScore::try_new(8).expect("score"),
                target: PillarScore::try_new(10).expect("target"),
                passing: PillarScore::try_new(7).expect("passing"),
                rationale: "reason".into(),
                supporting_finding_ids: vec![finding.finding_id],
                idempotency_key: None,
            },
        )
        .await
        .expect_err("must reject note severity linkage");
    assert!(
        err.to_string().contains("severity fix_now or defer"),
        "unexpected error: {err}"
    );
}

#[test]
fn renderer_error_carries_stable_code_and_file_line() {
    use tanren_app_services::methodology::source::{
        CommandFamily, CommandFrontmatter, CommandSource,
    };

    let src = CommandSource {
        name: "do-task".into(),
        family: CommandFamily::SpecLoop,
        frontmatter: CommandFrontmatter {
            name: "do-task".into(),
            role: "impl".into(),
            orchestration_loop: true,
            autonomy: "autonomous".into(),
            declared_variables: vec!["HOOK".into()],
            declared_tools: vec![],
            required_capabilities: vec![],
            produces_evidence: vec![],
            extras: Default::default(),
        },
        body: "preamble\nrun {{HOOK}} now\nthen {{MISSING}} later".into(),
        source_path: PathBuf::from("commands/do-task.md"),
    };
    let mut ctx = HashMap::new();
    ctx.insert("HOOK".into(), "just check".into());
    let err = render_command(&src, &ctx).expect_err("expected undeclared-var error");
    let msg = err.to_string();
    assert!(
        msg.contains("TANREN_RENDER_UNDECLARED_VAR"),
        "error missing stable code: {msg}"
    );
    assert!(msg.contains("MISSING"), "error missing variable: {msg}");
    assert!(
        msg.contains("commands/do-task.md:3:"),
        "error missing file:line:col — got {msg}"
    );
}

#[test]
fn renderer_error_on_unresolved_var_points_at_line() {
    use tanren_app_services::methodology::source::{
        CommandFamily, CommandFrontmatter, CommandSource,
    };
    let src = CommandSource {
        name: "demo".into(),
        family: CommandFamily::SpecLoop,
        frontmatter: CommandFrontmatter {
            name: "demo".into(),
            role: "impl".into(),
            orchestration_loop: false,
            autonomy: "autonomous".into(),
            declared_variables: vec!["NEED".into()],
            declared_tools: vec![],
            required_capabilities: vec![],
            produces_evidence: vec![],
            extras: Default::default(),
        },
        body: "header\n\n{{NEED}}\n".into(),
        source_path: PathBuf::from("commands/demo.md"),
    };
    let ctx = HashMap::new();
    let err = render_command(&src, &ctx).expect_err("unresolved");
    let msg = err.to_string();
    assert!(msg.contains("TANREN_RENDER_UNKNOWN_VAR"), "code: {msg}");
    assert!(msg.contains("commands/demo.md:3:"), "loc: {msg}");
}

#[test]
fn methodology_profile_override_applies_required_guards() {
    let base = tanren_app_services::methodology::config::MethodologyConfig {
        task_complete_requires: vec![
            RequiredGuard::GateChecked,
            RequiredGuard::Audited,
            RequiredGuard::Adherent,
        ],
        source: tanren_app_services::methodology::config::SourceConfig {
            path: PathBuf::from("commands"),
        },
        install_targets: vec![],
        mcp: Default::default(),
        variables: Default::default(),
        profiles: Default::default(),
    };
    let mut active = base.clone();
    let profile = MethodologyProfile {
        task_complete_requires: Some(vec![RequiredGuard::GateChecked]),
        ..Default::default()
    };
    profile.apply(&mut active);
    assert_eq!(active.task_complete_requires.len(), 1);
    assert_eq!(base.task_complete_requires.len(), 3);
}
