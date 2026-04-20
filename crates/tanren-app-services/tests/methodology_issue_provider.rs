use std::sync::Arc;

use tanren_app_services::methodology::{
    CapabilityScope, MethodologyService, PhaseId, ToolCapability,
};
use tanren_contract::methodology::CreateIssueParams;
use tanren_domain::SpecId;
use tanren_domain::methodology::issue::IssuePriority;
use tanren_store::Store;

async fn mk_service(issue_provider: &str) -> MethodologyService {
    let store = Store::open_and_migrate("sqlite::memory:?cache=shared")
        .await
        .expect("open");
    let runtime = tanren_app_services::methodology::service::PhaseEventsRuntime {
        spec_id: SpecId::new(),
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-methodology-provider-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "test-session".into(),
    };
    MethodologyService::with_runtime_and_pillars_and_issue_provider(
        Arc::new(store),
        vec![],
        Some(runtime),
        vec![],
        vec![],
        issue_provider,
    )
}

fn phase(tag: &str) -> PhaseId {
    PhaseId::try_new(tag).expect("phase")
}

fn scope(caps: &[ToolCapability]) -> CapabilityScope {
    CapabilityScope::from_iter_caps(caps.iter().copied())
}

#[tokio::test]
async fn create_issue_rejects_unsupported_configured_issue_provider() {
    let svc = mk_service("Linear").await;
    let spec_id = svc.phase_events_runtime().expect("runtime").spec_id;
    let err = svc
        .create_issue(
            &scope(&[ToolCapability::IssueCreate]),
            &phase("handle-feedback"),
            CreateIssueParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                origin_spec_id: spec_id,
                title: "issue".into(),
                description: String::new(),
                suggested_spec_scope: "scope".into(),
                priority: IssuePriority::Low,
                idempotency_key: None,
            },
        )
        .await
        .expect_err("unsupported provider must fail");
    assert!(matches!(
        err,
        tanren_app_services::methodology::MethodologyError::FieldValidation { field_path, .. }
            if field_path == "/issue_provider"
    ));
}
