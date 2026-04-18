//! Integration tests for the Lane 0.5 audit-remediation changes.
//!
//! Coverage (maps 1:1 to audit findings):
//! - config-driven required-guard set (#1)
//! - canonical-guard-name mapping (#13)
//! - idempotency_key field on `TaskGuardSatisfied` (#3)
//! - `TaskCompleted` converges when config guards satisfied (#2)
//! - relevance filter with explainable reasons (#5)
//! - renderer diagnostic carries file:line + stable error code (#7)
//! - typed replay error preserved through the service boundary (#10)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tanren_app_services::methodology::renderer::render_command;
use tanren_app_services::methodology::{
    CapabilityScope, MethodologyService, config::MethodologyProfile,
};
use tanren_contract::methodology::{
    CreateTaskParams, ListRelevantStandardsParams, RelevantStandard,
};
use tanren_domain::methodology::events::{MethodologyEvent, TaskGuardSatisfied};
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin, TaskStatus};
use tanren_domain::{SpecId, TaskId};
use tanren_store::Store;

async fn mk_service(required: Vec<RequiredGuard>) -> MethodologyService {
    let url = "sqlite::memory:?cache=shared";
    let store = Store::open_and_migrate(url).await.expect("open");
    MethodologyService::with_required_guards(Arc::new(store), required)
}

fn admin_scope() -> CapabilityScope {
    use tanren_domain::methodology::capability::ToolCapability::{
        FindingAdd, PhaseEscalate, PhaseOutcome, RubricRecord, SignpostAdd, StandardRead,
        TaskAbandon, TaskComplete, TaskCreate, TaskRead, TaskRevise, TaskStart,
    };
    CapabilityScope::from_iter_caps([
        TaskCreate,
        TaskStart,
        TaskComplete,
        TaskRevise,
        TaskAbandon,
        TaskRead,
        FindingAdd,
        RubricRecord,
        SignpostAdd,
        StandardRead,
        PhaseOutcome,
        PhaseEscalate,
    ])
}

#[tokio::test]
async fn required_guards_come_from_config_not_hardcoded() {
    // Two services with different configs; the list_tasks projection
    // must respect each service's guard set.
    let default = mk_service(vec![
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
        RequiredGuard::Adherent,
    ])
    .await;
    let relaxed = mk_service(vec![RequiredGuard::GateChecked]).await;
    assert_eq!(default.required_guards().len(), 3);
    assert_eq!(relaxed.required_guards().len(), 1);
    assert_eq!(relaxed.required_guards()[0], RequiredGuard::GateChecked);
}

#[tokio::test]
async fn config_driven_guards_dedup_on_construction() {
    let svc = mk_service(vec![
        RequiredGuard::GateChecked,
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
    ])
    .await;
    assert_eq!(svc.required_guards().len(), 2);
}

#[test]
fn canonical_guard_name_mapping_is_stable() {
    let spec = SpecId::new();
    let tid = TaskId::new();
    let cases = [
        (RequiredGuard::GateChecked, "TaskGateChecked"),
        (RequiredGuard::Audited, "TaskAudited"),
        (RequiredGuard::Adherent, "TaskAdherent"),
        (RequiredGuard::Extra("perf_checked".into()), "TaskXChecked"),
    ];
    for (guard, expected) in cases {
        let ev = TaskGuardSatisfied {
            task_id: tid,
            spec_id: spec,
            guard,
            idempotency_key: Some("hash".into()),
        };
        assert_eq!(ev.canonical_event_name(), expected);
        // `MethodologyEvent::TaskGuardSatisfied` is the stored shape;
        // the canonical name comes from a helper, not an enum variant.
        let wrapped = MethodologyEvent::TaskGuardSatisfied(ev.clone());
        assert!(matches!(wrapped, MethodologyEvent::TaskGuardSatisfied(_)));
    }
}

#[test]
fn idempotency_key_serializes_and_round_trips() {
    let ev = TaskGuardSatisfied {
        task_id: TaskId::new(),
        spec_id: SpecId::new(),
        guard: RequiredGuard::GateChecked,
        idempotency_key: Some("blake3:abc".into()),
    };
    let json =
        serde_json::to_string(&MethodologyEvent::TaskGuardSatisfied(ev.clone())).expect("ser");
    assert!(json.contains("idempotency_key"));
    let back: MethodologyEvent = serde_json::from_str(&json).expect("de");
    let MethodologyEvent::TaskGuardSatisfied(decoded) = back else {
        unreachable!("wrong variant after round-trip");
    };
    assert_eq!(decoded, ev);
}

#[test]
fn idempotency_key_absent_in_json_when_none() {
    let ev = TaskGuardSatisfied {
        task_id: TaskId::new(),
        spec_id: SpecId::new(),
        guard: RequiredGuard::Audited,
        idempotency_key: None,
    };
    let json = serde_json::to_string(&MethodologyEvent::TaskGuardSatisfied(ev)).expect("ser");
    // skip_serializing_if keeps the wire shape clean for legacy callers.
    assert!(
        !json.contains("idempotency_key"),
        "expected idempotency_key omitted when None, got: {json}"
    );
}

#[tokio::test]
async fn mark_guard_satisfied_fires_task_completed_when_config_satisfied() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let spec_id = SpecId::new();
    let resp = svc
        .create_task(
            &scope,
            "do-task",
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
    svc.start_task(
        &scope,
        "do-task",
        tanren_contract::methodology::StartTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
        },
    )
    .await
    .expect("start");
    svc.complete_task(
        &scope,
        "do-task",
        tanren_contract::methodology::CompleteTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
            evidence_refs: vec![],
        },
    )
    .await
    .expect("implement");
    svc.mark_task_guard_satisfied(
        &scope,
        "do-task",
        resp.task_id,
        RequiredGuard::GateChecked,
        Some("test-idem".into()),
    )
    .await
    .expect("mark guard");

    // Re-fold via list_tasks; the task should now be `Complete`.
    let tasks = svc
        .list_tasks(
            &scope,
            "do-task",
            tanren_contract::methodology::ListTasksParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: Some(spec_id),
            },
        )
        .await
        .expect("list");
    let task = tasks.iter().find(|t| t.id == resp.task_id).expect("task");
    assert_eq!(task.status, TaskStatus::Complete);
}

#[tokio::test]
async fn mark_guard_satisfied_keeps_implemented_when_guard_not_required() {
    // Service configured to require Audited; firing GateChecked alone
    // must leave the task at Implemented, not Complete.
    let svc = mk_service(vec![RequiredGuard::Audited]).await;
    let scope = admin_scope();
    let spec_id = SpecId::new();
    let resp = svc
        .create_task(
            &scope,
            "do-task",
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
    svc.start_task(
        &scope,
        "do-task",
        tanren_contract::methodology::StartTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
        },
    )
    .await
    .expect("start");
    svc.complete_task(
        &scope,
        "do-task",
        tanren_contract::methodology::CompleteTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: resp.task_id,
            evidence_refs: vec![],
        },
    )
    .await
    .expect("implement");
    svc.mark_task_guard_satisfied(
        &scope,
        "do-task",
        resp.task_id,
        RequiredGuard::GateChecked,
        None,
    )
    .await
    .expect("mark non-required");

    let tasks = svc
        .list_tasks(
            &scope,
            "do-task",
            tanren_contract::methodology::ListTasksParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: Some(spec_id),
            },
        )
        .await
        .expect("list");
    let task = tasks.iter().find(|t| t.id == resp.task_id).expect("task");
    assert!(
        matches!(task.status, TaskStatus::Implemented { .. }),
        "expected Implemented, got {:?}",
        task.status
    );
}

#[tokio::test]
async fn relevance_filter_explains_inclusion_by_touched_files() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let scope = admin_scope();
    let out: Vec<RelevantStandard> = svc
        .list_relevant_standards_filtered(
            &scope,
            "adhere-task",
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                touched_files: vec!["crates/tanren-domain/src/lib.rs".into()],
                project_language: Some("rust".into()),
                domains: vec![],
            },
        )
        .expect("filtered");
    assert!(!out.is_empty(), "rust-touched file should match >=1 std");
    assert!(
        out.iter().all(|r| !r.inclusion_reason.is_empty()),
        "every kept standard must carry an explanation"
    );
}

#[tokio::test]
async fn relevance_filter_empty_inputs_returns_full_baseline() {
    let svc = mk_service(vec![]).await;
    let scope = admin_scope();
    let out = svc
        .list_relevant_standards_filtered(
            &scope,
            "adhere-task",
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                touched_files: vec![],
                project_language: None,
                domains: vec![],
            },
        )
        .expect("baseline");
    assert!(!out.is_empty());
    // The fallback reason must identify this as the upper bound.
    assert!(out.iter().any(|r| r.inclusion_reason.contains("baseline")));
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
