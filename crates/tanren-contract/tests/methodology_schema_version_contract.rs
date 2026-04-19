use schemars::JsonSchema;
use schemars::schema_for;
use tanren_contract::methodology::{
    AckResponse, AddFindingResponse, AddSignpostResponse, CreateIssueResponse, CreateTaskResponse,
    ListRelevantStandardsResponse, ListTasksResponse, METHODOLOGY_SCHEMA_VERSION, RelevantStandard,
    SchemaVersion,
};
use tanren_domain::methodology::standard::{Standard, StandardImportance};
use tanren_domain::{FindingId, IssueId, NonEmptyString, SignpostId, TaskId};

fn assert_schema_version_required<T: JsonSchema>() {
    let schema_json = serde_json::to_value(schema_for!(T)).expect("schema json");
    let required = schema_json
        .get("required")
        .and_then(serde_json::Value::as_array)
        .expect("response schema required[]");
    let properties = schema_json
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .expect("response schema properties");
    assert!(
        required.iter().any(|v| v == "schema_version"),
        "schema_version must be required"
    );
    assert!(
        properties.contains_key("schema_version"),
        "schema_version must be declared"
    );
}

fn assert_serialized_schema_version<T: serde::Serialize>(value: T) {
    let body = serde_json::to_value(value).expect("serialize");
    assert_eq!(
        body.get("schema_version")
            .and_then(serde_json::Value::as_str),
        Some(METHODOLOGY_SCHEMA_VERSION)
    );
}

#[test]
fn every_methodology_response_schema_requires_schema_version() {
    assert_schema_version_required::<AckResponse>();
    assert_schema_version_required::<CreateTaskResponse>();
    assert_schema_version_required::<ListTasksResponse>();
    assert_schema_version_required::<AddFindingResponse>();
    assert_schema_version_required::<AddSignpostResponse>();
    assert_schema_version_required::<CreateIssueResponse>();
    assert_schema_version_required::<ListRelevantStandardsResponse>();
    assert_schema_version_required::<RelevantStandard>();
}

#[test]
fn every_methodology_response_payload_serializes_schema_version() {
    let schema_version = SchemaVersion::current();
    assert_serialized_schema_version(AckResponse {
        schema_version: schema_version.clone(),
    });
    assert_serialized_schema_version(CreateTaskResponse {
        schema_version: schema_version.clone(),
        task_id: TaskId::new(),
    });
    assert_serialized_schema_version(ListTasksResponse {
        schema_version: schema_version.clone(),
        tasks: Vec::new(),
    });
    assert_serialized_schema_version(AddFindingResponse {
        schema_version: schema_version.clone(),
        finding_id: FindingId::new(),
    });
    assert_serialized_schema_version(AddSignpostResponse {
        schema_version: schema_version.clone(),
        signpost_id: SignpostId::new(),
    });
    assert_serialized_schema_version(CreateIssueResponse {
        schema_version: schema_version.clone(),
        issue_id: IssueId::new(),
        reference: tanren_domain::methodology::issue::IssueRef {
            provider: tanren_domain::methodology::issue::IssueProvider::GitHub,
            number: 1,
            url: NonEmptyString::try_new("https://example.invalid/issues/1").expect("url"),
        },
    });
    assert_serialized_schema_version(RelevantStandard {
        schema_version: schema_version.clone(),
        standard: Standard {
            name: NonEmptyString::try_new("no-unwrap-in-production").expect("name"),
            category: NonEmptyString::try_new("rust-error-handling").expect("category"),
            applies_to: vec!["**/*.rs".into()],
            applies_to_languages: vec!["rust".into()],
            applies_to_domains: vec!["error-handling".into()],
            importance: StandardImportance::Critical,
            body: "no unwrap".into(),
        },
        inclusion_reason: "matched applies_to".into(),
    });
    assert_serialized_schema_version(ListRelevantStandardsResponse {
        schema_version,
        standards: Vec::new(),
    });
}
