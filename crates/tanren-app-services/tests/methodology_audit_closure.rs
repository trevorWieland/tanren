use std::sync::Arc;

use tanren_app_services::methodology::{CapabilityScope, MethodologyService, PhaseEventLine};
use tanren_contract::methodology::{
    CreateIssueParams, CreateTaskParams, EscalateToBlockerParams, ListRelevantStandardsParams,
    PostReplyDirectiveParams, RecordAdherenceFindingParams, SetSpecRelevanceContextParams,
};
use tanren_domain::methodology::capability::ToolCapability;
use tanren_domain::methodology::event_tool::PhaseEventOriginKind;
use tanren_domain::methodology::events::{MethodologyEvent, TaskGateChecked};
use tanren_domain::methodology::finding::{AdherenceSeverity, StandardRef};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::spec::SpecRelevanceContext;
use tanren_domain::methodology::standard::{Standard, StandardImportance};
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin};
use tanren_domain::{NonEmptyString, SpecId};
use tanren_store::Store;

async fn mk_service(required: Vec<RequiredGuard>) -> MethodologyService {
    mk_service_with_standards(required, vec![]).await
}

async fn mk_service_with_standards(
    required: Vec<RequiredGuard>,
    standards: Vec<Standard>,
) -> MethodologyService {
    let store = Store::open_and_migrate("sqlite::memory:?cache=shared")
        .await
        .expect("open");
    let runtime = tanren_app_services::methodology::service::PhaseEventsRuntime {
        spec_id: SpecId::new(),
        spec_folder: std::env::temp_dir()
            .join(format!("tanren-methodology-audit-{}", uuid::Uuid::now_v7())),
        agent_session_id: "test-session".into(),
    };
    MethodologyService::with_runtime(Arc::new(store), required, Some(runtime), standards)
}

fn scope(caps: &[ToolCapability]) -> CapabilityScope {
    CapabilityScope::from_iter_caps(caps.iter().copied())
}

fn phase(tag: &str) -> PhaseId {
    PhaseId::try_new(tag).expect("phase")
}

fn runtime_spec_id(svc: &MethodologyService) -> SpecId {
    svc.phase_events_runtime().expect("runtime").spec_id
}

fn phase_events(svc: &MethodologyService) -> Vec<PhaseEventLine> {
    let runtime = svc.phase_events_runtime().expect("runtime");
    let path = runtime.spec_folder.join("phase-events.jsonl");
    let raw = std::fs::read_to_string(path).expect("read phase events");
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<PhaseEventLine>(line).expect("phase event line"))
        .collect()
}

#[tokio::test]
async fn adherence_rejects_unknown_standard_refs() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let err = svc
        .record_adherence_finding(
            &scope(&[ToolCapability::AdherenceRecord]),
            &phase("adhere-task"),
            RecordAdherenceFindingParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: runtime_spec_id(&svc),
                standard: StandardRef {
                    name: NonEmptyString::try_new("missing-standard").expect("name"),
                    category: NonEmptyString::try_new("missing-category").expect("category"),
                },
                affected_files: vec!["src/lib.rs".into()],
                line_numbers: vec![7],
                severity: AdherenceSeverity::FixNow,
                rationale: "not in registry".into(),
                idempotency_key: None,
            },
        )
        .await
        .expect_err("unknown standard must fail");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::FieldValidation { field_path, .. }
            if field_path == "/standard"
    ));
}

#[tokio::test]
async fn adherence_rejects_defer_for_critical_standard() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let err = svc
        .record_adherence_finding(
            &scope(&[ToolCapability::AdherenceRecord]),
            &phase("adhere-task"),
            RecordAdherenceFindingParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: runtime_spec_id(&svc),
                standard: StandardRef {
                    name: NonEmptyString::try_new("no-unwrap-in-production").expect("name"),
                    category: NonEmptyString::try_new("rust-error-handling").expect("category"),
                },
                affected_files: vec!["src/lib.rs".into()],
                line_numbers: vec![9],
                severity: AdherenceSeverity::Defer,
                rationale: "defer critical rule".into(),
                idempotency_key: None,
            },
        )
        .await
        .expect_err("critical defer must fail");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::FieldValidation { field_path, .. }
            if field_path == "/severity"
    ));
}

#[tokio::test]
async fn relevance_filters_use_server_derived_context_and_hints_are_additive() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let spec_id = runtime_spec_id(&svc);
    let scope = scope(&[
        ToolCapability::SpecFrontmatter,
        ToolCapability::StandardRead,
    ]);
    svc.set_spec_relevance_context(
        &scope,
        &phase("shape-spec"),
        SetSpecRelevanceContextParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            spec_id,
            relevance_context: SpecRelevanceContext {
                touched_files: vec!["src/lib.rs".into()],
                project_language: Some("rust".into()),
                tags: vec!["security".into()],
                category: Some("backend".into()),
            },
            idempotency_key: None,
        },
    )
    .await
    .expect("set relevance context");

    let derived_only = svc
        .list_relevant_standards_filtered(
            &scope,
            &phase("adhere-spec"),
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                touched_files: vec![],
                project_language: None,
                domains: vec![],
                tags: vec![],
                category: None,
            },
        )
        .await
        .expect("derived only");
    let with_conflicting_hints = svc
        .list_relevant_standards_filtered(
            &scope,
            &phase("adhere-spec"),
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                touched_files: vec!["src/main.py".into()],
                project_language: Some("python".into()),
                domains: vec!["frontend".into()],
                tags: vec!["frontend".into()],
                category: Some("frontend".into()),
            },
        )
        .await
        .expect("with hints");

    let mut a = derived_only
        .standards
        .into_iter()
        .map(|s| format!("{}:{}", s.standard.category, s.standard.name))
        .collect::<Vec<_>>();
    let mut b = with_conflicting_hints
        .standards
        .into_iter()
        .map(|s| format!("{}:{}", s.standard.category, s.standard.name))
        .collect::<Vec<_>>();
    a.sort();
    b.sort();
    assert_eq!(
        a, b,
        "caller hints must be additive and must not narrow server-derived relevance"
    );
}

#[tokio::test]
async fn globset_matches_complex_patterns_with_path_normalization() {
    let custom_standard = Standard {
        name: NonEmptyString::try_new("mod-wildcard").expect("name"),
        category: NonEmptyString::try_new("custom").expect("category"),
        applies_to: vec!["src/**/mod?.rs".into()],
        applies_to_languages: vec![],
        applies_to_domains: vec![],
        importance: StandardImportance::High,
        body: "custom".into(),
    };
    let svc = mk_service_with_standards(vec![], vec![custom_standard]).await;
    let out = svc
        .list_relevant_standards_filtered(
            &scope(&[ToolCapability::StandardRead]),
            &phase("adhere-task"),
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                touched_files: vec!["src\\core\\mod1.rs".into()],
                project_language: None,
                domains: vec![],
                tags: vec![],
                category: None,
            },
        )
        .await
        .expect("filtered");
    assert_eq!(
        out.standards.len(),
        1,
        "globset pattern should match normalized path"
    );
    assert!(
        out.standards[0]
            .inclusion_reason
            .contains("matched `applies_to`"),
        "expected applies_to inclusion reason, got {}",
        out.standards[0].inclusion_reason
    );
}

#[tokio::test]
async fn guard_plus_completion_events_share_causal_link_and_origin_kind() {
    let svc = mk_service(vec![RequiredGuard::GateChecked]).await;
    let spec_id = runtime_spec_id(&svc);
    let task_scope = scope(&[
        ToolCapability::TaskCreate,
        ToolCapability::TaskStart,
        ToolCapability::TaskComplete,
    ]);
    let created = svc
        .create_task(
            &task_scope,
            &phase("do-task"),
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
        &task_scope,
        &phase("do-task"),
        tanren_contract::methodology::StartTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: created.task_id,
        },
    )
    .await
    .expect("start");
    svc.complete_task(
        &task_scope,
        &phase("do-task"),
        tanren_contract::methodology::CompleteTaskParams {
            schema_version: tanren_contract::methodology::SchemaVersion::current(),
            idempotency_key: None,
            task_id: created.task_id,
            evidence_refs: vec![],
        },
    )
    .await
    .expect("implement");
    svc.mark_task_guard_satisfied(
        &task_scope,
        &phase("do-task"),
        created.task_id,
        RequiredGuard::GateChecked,
        Some("call-guard-1".into()),
    )
    .await
    .expect("guard");

    let lines = phase_events(&svc);
    let guard = lines
        .iter()
        .rev()
        .find(|line| {
            matches!(
                line.payload,
                MethodologyEvent::TaskGateChecked(TaskGateChecked { task_id, .. })
                    if task_id == created.task_id
            )
        })
        .expect("task gate line");
    let completed = lines
        .iter()
        .rev()
        .find(|line| {
            matches!(line.payload, MethodologyEvent::TaskCompleted(ref ev) if ev.task_id == created.task_id)
        })
        .expect("task completed line");

    assert_eq!(guard.tool, "mark_task_guard_satisfied");
    assert_eq!(completed.tool, "mark_task_guard_satisfied");
    assert_eq!(
        guard.caused_by_tool_call_id.as_deref(),
        Some("call-guard-1")
    );
    assert_eq!(
        completed.caused_by_tool_call_id.as_deref(),
        Some("call-guard-1")
    );
    assert_eq!(guard.origin_kind, PhaseEventOriginKind::ToolPrimary);
    assert_eq!(completed.origin_kind, PhaseEventOriginKind::ToolDerived);
}

#[tokio::test]
async fn create_issue_requires_typed_allowed_phases() {
    let svc = mk_service(vec![]).await;
    let err = svc
        .create_issue(
            &scope(&[ToolCapability::IssueCreate]),
            &phase("do-task"),
            CreateIssueParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                origin_spec_id: SpecId::new(),
                title: "issue".into(),
                description: String::new(),
                suggested_spec_scope: "scope".into(),
                priority: tanren_domain::methodology::issue::IssuePriority::Low,
                idempotency_key: None,
            },
        )
        .await
        .expect_err("phase restriction");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::FieldValidation { field_path, .. }
            if field_path == "/phase"
    ));
}

#[tokio::test]
async fn escalate_to_blocker_requires_investigate_phase() {
    let svc = mk_service(vec![]).await;
    let err = svc
        .escalate_to_blocker(
            &scope(&[ToolCapability::PhaseEscalate]),
            &phase("handle-feedback"),
            EscalateToBlockerParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                reason: "blocked".into(),
                options: vec![],
                idempotency_key: None,
            },
        )
        .await
        .expect_err("phase restriction");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::FieldValidation { field_path, .. }
            if field_path == "/phase"
    ));
}

#[tokio::test]
async fn post_reply_directive_requires_handle_feedback_phase() {
    let svc = mk_service(vec![]).await;
    let err = svc
        .post_reply_directive(
            &scope(&[ToolCapability::FeedbackReply]),
            &phase("investigate"),
            PostReplyDirectiveParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                thread_ref: "thread-1".into(),
                body: "body".into(),
                disposition: tanren_domain::methodology::phase_outcome::ReplyDisposition::Ack,
                idempotency_key: None,
            },
        )
        .await
        .expect_err("phase restriction");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::FieldValidation { field_path, .. }
            if field_path == "/phase"
    ));
}

#[test]
fn service_tool_paths_forbid_string_validation_variant() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/methodology");
    let mut checked = 0_u32;
    for entry in std::fs::read_dir(&root).expect("read methodology dir") {
        let path = entry.expect("entry").path();
        let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
            continue;
        };
        if !name.starts_with("service")
            || path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs")
        {
            continue;
        }
        checked = checked.saturating_add(1);
        let content = std::fs::read_to_string(&path).expect("read service module");
        assert!(
            !content.contains("MethodologyError::Validation("),
            "service module `{}` regressed to stringly validation; use FieldValidation instead",
            path.display()
        );
    }
    assert!(checked > 0, "expected at least one service module");
}
