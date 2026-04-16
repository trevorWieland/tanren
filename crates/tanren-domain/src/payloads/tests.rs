use super::*;

#[test]
fn config_env_debug_redacts_values() {
    let mut map = HashMap::new();
    map.insert("API_URL".to_string(), "https://example.test".to_string());
    map.insert("SECRET_TOKEN".to_string(), "super-secret-value".to_string());
    let env = ConfigEnv::new(map);
    let dbg = format!("{env:?}");
    assert!(dbg.contains("API_URL"));
    assert!(dbg.contains("SECRET_TOKEN"));
    assert!(!dbg.contains("super-secret-value"));
    assert!(!dbg.contains("https://example.test"));
}

#[test]
fn config_env_to_keys_returns_sorted_unique_keys() {
    let mut map = HashMap::new();
    map.insert("ZETA".to_string(), "z".to_string());
    map.insert("ALPHA".to_string(), "a".to_string());
    map.insert("MIDDLE".to_string(), "m".to_string());
    let keys = ConfigEnv::new(map).to_keys();
    assert_eq!(keys.as_slice(), &["ALPHA", "MIDDLE", "ZETA"]);
}

#[test]
fn config_keys_deduplicates() {
    let keys = ConfigKeys::from_strings(vec![
        "B".to_string(),
        "A".to_string(),
        "B".to_string(),
        "C".to_string(),
    ]);
    assert_eq!(keys.as_slice(), &["A", "B", "C"]);
}

#[test]
fn config_keys_serde_roundtrip() {
    let keys = ConfigKeys::from_strings(["API_URL".to_string(), "BUILD_TAG".to_string()]);
    let json = serde_json::to_string(&keys).expect("serialize");
    assert_eq!(json, "[\"API_URL\",\"BUILD_TAG\"]");
    let back: ConfigKeys = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(keys, back);
}

#[test]
fn environment_handle_has_no_runtime_data_field() {
    // Compile-time check: EnvironmentHandle is {id, runtime_type}.
    let handle = EnvironmentHandle {
        id: NonEmptyString::try_new("env-1").expect("valid"),
        runtime_type: NonEmptyString::try_new("docker").expect("valid"),
    };
    let json = serde_json::to_string(&handle).expect("serialize");
    assert!(!json.contains("runtime_data"));
}

#[test]
fn finding_severity_display_matches_serde() {
    for (severity, tag) in [
        (FindingSeverity::Fix, "fix"),
        (FindingSeverity::Note, "note"),
        (FindingSeverity::Question, "question"),
    ] {
        assert_eq!(severity.to_string(), tag);
        let json = serde_json::to_string(&severity).expect("serialize");
        assert_eq!(json, format!("\"{tag}\""));
    }
}

#[test]
fn execute_result_rejects_non_finite_duration() {
    // A caller trying to build an ExecuteResult with a non-finite
    // duration must go through FiniteF64::try_new and see an
    // explicit error instead of silent `null` serialization.
    assert!(FiniteF64::try_new(f64::NAN).is_err());
    assert!(FiniteF64::try_new(f64::INFINITY).is_err());

    // The valid-construction path still works.
    let result = ExecuteResult {
        outcome: Outcome::Success,
        signal: None,
        exit_code: Some(0),
        duration_secs: FiniteF64::try_new(1.5).expect("finite"),
        gate_output: None,
        tail_output: None,
        stderr_tail: None,
        pushed: false,
        plan_hash: None,
        unchecked_tasks: 0,
        spec_modified: false,
        findings: vec![],
        token_usage: None,
    };
    let json = serde_json::to_string(&result).expect("serialize");
    let back: ExecuteResult = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(result, back);

    // And round-trips through the SeaORM Value path too.
    let value = serde_json::to_value(&result).expect("to_value");
    let from_value: ExecuteResult = serde_json::from_value(value).expect("from_value");
    assert_eq!(result, from_value);
}

#[test]
fn teardown_result_option_cost_accepts_none_and_finite() {
    // None is allowed.
    let none = TeardownResult {
        vm_released: true,
        duration_secs: FiniteF64::try_new(2.0).expect("finite"),
        estimated_cost: None,
    };
    let json = serde_json::to_string(&none).expect("serialize");
    let back: TeardownResult = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(none, back);

    // Some(finite) round-trips through both paths.
    let some = TeardownResult {
        vm_released: true,
        duration_secs: FiniteF64::try_new(2.0).expect("finite"),
        estimated_cost: Some(FiniteF64::try_new(0.12).expect("finite")),
    };
    let value = serde_json::to_value(some).expect("to_value");
    let from_value: TeardownResult = serde_json::from_value(value).expect("from_value");
    assert_eq!(some, from_value);
}

fn sample_snapshot() -> DispatchSnapshot {
    DispatchSnapshot {
        project: NonEmptyString::try_new("proj".to_owned()).expect("project"),
        phase: Phase::DoTask,
        cli: Cli::Claude,
        auth_mode: AuthMode::ApiKey,
        branch: NonEmptyString::try_new("main".to_owned()).expect("branch"),
        spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("spec"),
        workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("workflow"),
        timeout: TimeoutSecs::try_new(60).expect("timeout"),
        environment_profile: NonEmptyString::try_new("default".to_owned()).expect("profile"),
        gate_cmd: None,
        context: None,
        model: None,
        project_env: ConfigKeys::default(),
        required_secrets: vec![],
        preserve_on_failure: false,
        created_at: Utc::now(),
    }
}

#[test]
fn provision_ref_payload_roundtrips() {
    let payload = StepPayload::ProvisionRef(Box::new(ProvisionRefPayload {
        dispatch_ref: DispatchSnapshotRef::new(DispatchId::new()),
    }));
    let json = serde_json::to_string(&payload).expect("serialize");
    let back: StepPayload = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(payload, back);
}

#[test]
fn legacy_embedded_provision_payload_still_deserializes() {
    let payload = StepPayload::Provision(Box::new(ProvisionPayload {
        dispatch: sample_snapshot(),
    }));
    let json = serde_json::to_string(&payload).expect("serialize");
    let back: StepPayload = serde_json::from_str(&json).expect("deserialize");
    assert!(matches!(back, StepPayload::Provision(_)));
}
