use proptest::prelude::*;

use super::*;

#[test]
fn typed_code_is_the_only_terminal_classification_input() {
    let class = classify_provider_failure(&ProviderFailureContext {
        typed_code: ProviderFailureCode::Timeout,
        provider_code: Some(ProviderIdentifier::try_new("rate_limited").expect("code")),
        provider_kind: None,
        signal: None,
        exit_code: None,
        stdout_tail: None,
        stderr_tail: Some("401 invalid api key".into()),
    });
    assert_eq!(class, HarnessFailureClass::Timeout);
}

#[test]
fn provider_failure_normalizes_through_context_classification() {
    let provider_failure =
        ProviderFailure::new(ProviderFailureCode::RateLimited, "raw adapter failure").with_context(
            ProviderFailureContext {
                typed_code: ProviderFailureCode::RateLimited,
                provider_code: None,
                provider_kind: None,
                signal: None,
                exit_code: None,
                stdout_tail: None,
                stderr_tail: Some("fatal panic".into()),
            },
        );

    let failure = provider_failure.into_harness_failure();
    assert_eq!(failure.class(), HarnessFailureClass::RateLimited);
    assert_eq!(failure.typed_code(), ProviderFailureCode::RateLimited);
}

#[test]
fn audit_classifier_uses_structured_and_text_fallback_only_for_unknown_typed_code() {
    let class = classify_provider_failure_for_audit(&AuditProviderFailureContext {
        typed_code: AuditProviderFailureCode::Unknown,
        provider_code: Some(ProviderIdentifier::try_new("rate_limited").expect("code")),
        provider_kind: None,
        signal: None,
        exit_code: None,
        stdout_tail: None,
        stderr_tail: None,
    });
    assert_eq!(class, HarnessFailureClass::RateLimited);

    let from_text = classify_provider_failure_for_audit(&AuditProviderFailureContext {
        typed_code: AuditProviderFailureCode::Unknown,
        provider_code: None,
        provider_kind: None,
        signal: None,
        exit_code: None,
        stdout_tail: None,
        stderr_tail: Some("429 too many requests".into()),
    });
    assert_eq!(from_text, HarnessFailureClass::RateLimited);
}

#[test]
fn maps_rate_limit_to_transient_domain_error_class() {
    let class = classify_provider_failure(&ProviderFailureContext::new(
        ProviderFailureCode::RateLimited,
    ));
    assert_eq!(class, HarnessFailureClass::RateLimited);
    assert_eq!(class.to_domain_error_class(), ErrorClass::Transient);
}

#[test]
fn maps_capability_denial_to_fatal_domain_error_class() {
    let class = classify_provider_failure(&ProviderFailureContext::new(
        ProviderFailureCode::CapabilityDenied,
    ));
    assert_eq!(class, HarnessFailureClass::CapabilityDenied);
    assert_eq!(class.to_domain_error_class(), ErrorClass::Fatal);
}

#[test]
fn provider_identifier_rejects_whitespace_and_invalid_chars() {
    let empty = ProviderIdentifier::try_new(" ").expect_err("must reject empty");
    assert_eq!(empty, ProviderIdentifierError::EmptyOrWhitespace);
    let invalid = ProviderIdentifier::try_new("bad value")
        .expect_err("space is not an allowed identifier character");
    assert_eq!(invalid, ProviderIdentifierError::InvalidCharacter);
}

#[test]
fn provider_run_id_rejects_whitespace_and_invalid_chars() {
    let empty = ProviderRunId::try_new(" ").expect_err("must reject empty");
    assert_eq!(empty, ProviderRunIdError::EmptyOrWhitespace);
    let invalid =
        ProviderRunId::try_new("bad value").expect_err("space is not an allowed run id character");
    assert_eq!(invalid, ProviderRunIdError::InvalidCharacter);
}

#[test]
fn deserialization_rejects_unknown_typed_code() {
    let payload = serde_json::json!({
        "class": "unknown",
        "message": "bad",
        "typed_code": "unknown"
    });
    let err = serde_json::from_value::<HarnessFailure>(payload).expect_err("must reject");
    let msg = err.to_string();
    assert!(msg.contains("unknown variant"), "{msg}");
}

proptest! {
    #[test]
    fn typed_codes_are_never_overridden_by_audit_fallback(noise in ".{0,120}") {
        let class = classify_provider_failure(&ProviderFailureContext {
            typed_code: ProviderFailureCode::Authentication,
            provider_code: None,
            provider_kind: None,
            signal: None,
            exit_code: None,
            stdout_tail: Some(noise),
            stderr_tail: Some("429 too many requests temporary".into()),
        });
        prop_assert_eq!(class, HarnessFailureClass::Authentication);
    }

    #[test]
    fn audit_fallback_still_classifies_unknown_typed_code_from_exit_code(noise in ".{0,120}") {
        let class = classify_provider_failure_for_audit(&AuditProviderFailureContext {
            typed_code: AuditProviderFailureCode::Unknown,
            provider_code: None,
            provider_kind: None,
            signal: None,
            exit_code: Some(75),
            stdout_tail: None,
            stderr_tail: Some(noise),
        });
        prop_assert_eq!(class, HarnessFailureClass::Transient);
    }
}
