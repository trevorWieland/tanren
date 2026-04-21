use super::*;

#[tokio::test]
async fn preserves_provider_run_id_when_it_is_safe() {
    let adapter = MockAdapter {
        output: raw_output(),
        provider_failure: None,
        provider_run_id: Some(ProviderRunId::try_new("run_12345:abc.DEF").expect("run id")),
    };
    let mut recorder = Recorder::default();
    let result = execute_with_contract(&adapter, &request(), &mut recorder)
        .await
        .expect("must succeed");
    assert_eq!(
        result.provider_run_id.as_ref().map(ProviderRunId::as_str),
        Some("run_12345:abc.DEF")
    );
}

#[tokio::test]
async fn fails_closed_when_provider_run_id_is_redaction_mutated() {
    let adapter = MockAdapter {
        output: raw_output(),
        provider_failure: None,
        provider_run_id: Some(ProviderRunId::try_new("run-sk-live-secret").expect("run id")),
    };
    let mut recorder = Recorder::default();
    let err = execute_with_contract(&adapter, &request(), &mut recorder)
        .await
        .expect_err("must fail closed");
    assert!(matches!(
        err,
        HarnessContractError::UnsafeProviderMetadata {
            field: "provider_run_id",
            violation: ProviderMetadataViolation::RedactedOrMutated
        }
    ));
}
