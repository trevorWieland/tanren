use std::sync::Arc;

use tanren_app_services::methodology::MethodologyService;
use tanren_contract::methodology::ListRelevantStandardsParams;
use tanren_domain::methodology::standard::{Standard, StandardImportance};
use tanren_domain::{NonEmptyString, SpecId};
use tanren_store::Store;

use super::{admin_scope, mk_service, phase, runtime_spec_id};

async fn mk_service_with_standards(standards: Vec<Standard>) -> MethodologyService {
    let url = "sqlite::memory:?cache=shared";
    let store = Store::open_and_migrate(url).await.expect("open");
    let runtime = tanren_app_services::methodology::service::PhaseEventsRuntime {
        spec_id: SpecId::new(),
        spec_folder: std::env::temp_dir().join(format!(
            "tanren-methodology-remediation-standards-{}",
            uuid::Uuid::now_v7()
        )),
        agent_session_id: "test-session".into(),
    };
    MethodologyService::with_runtime(Arc::new(store), vec![], Some(runtime), standards)
}

#[tokio::test]
async fn relevance_filter_explains_inclusion_by_touched_files() {
    let svc = mk_service(vec![]).await;
    let scope = admin_scope();
    let out = svc
        .list_relevant_standards_filtered(
            &scope,
            &phase("adhere-task"),
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                touched_files: vec!["crates/tanren-domain/src/lib.rs".into()],
                project_language: Some("rust".into()),
                domains: vec![],
                tags: vec![],
                category: None,
            },
        )
        .await
        .expect("filtered");
    assert!(
        !out.standards.is_empty(),
        "rust-touched file should match >=1 std"
    );
    assert!(
        out.standards.iter().all(|r| !r.inclusion_reason.is_empty()),
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
            &phase("adhere-task"),
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id: SpecId::new(),
                touched_files: vec![],
                project_language: None,
                domains: vec![],
                tags: vec![],
                category: None,
            },
        )
        .await
        .expect("baseline");
    assert!(!out.standards.is_empty());
    assert!(
        out.standards
            .iter()
            .any(|r| r.inclusion_reason.contains("baseline"))
    );
}

#[tokio::test]
async fn relevance_filter_normalizes_domain_and_language_case_consistently() {
    let standard = Standard {
        name: NonEmptyString::try_new("case-normalized-standard").expect("name"),
        category: NonEmptyString::try_new("custom").expect("category"),
        applies_to: vec![],
        applies_to_languages: vec!["Rust".into()],
        applies_to_domains: vec!["Architecture".into()],
        importance: StandardImportance::Medium,
        body: "test standard".into(),
    };
    let svc = mk_service_with_standards(vec![standard]).await;
    let scope = admin_scope();
    let spec_id = runtime_spec_id(&svc);

    let language_match = svc
        .list_relevant_standards_filtered(
            &scope,
            &phase("adhere-task"),
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                touched_files: vec![],
                project_language: Some("rUsT".into()),
                domains: vec![],
                tags: vec![],
                category: None,
            },
        )
        .await
        .expect("language relevance");
    assert_eq!(
        language_match.standards.len(),
        1,
        "mixed-case language should match case-insensitively"
    );

    let domain_match = svc
        .list_relevant_standards_filtered(
            &scope,
            &phase("adhere-task"),
            &ListRelevantStandardsParams {
                schema_version: tanren_contract::methodology::SchemaVersion::current(),
                spec_id,
                touched_files: vec![],
                project_language: None,
                domains: vec!["aRcHiTeCtUrE".into()],
                tags: vec![],
                category: None,
            },
        )
        .await
        .expect("domain relevance");
    assert_eq!(
        domain_match.standards.len(),
        1,
        "mixed-case domain should match case-insensitively"
    );
}
