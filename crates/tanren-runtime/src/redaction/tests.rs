use proptest::prelude::*;
use tanren_domain::Outcome;

use super::*;

fn raw_output(text: &str) -> RawExecutionOutput {
    RawExecutionOutput {
        outcome: Outcome::Success,
        signal: None,
        exit_code: Some(0),
        duration_secs: 1.0,
        gate_output: Some(text.into()),
        tail_output: Some(text.into()),
        stderr_tail: Some(text.into()),
        pushed: false,
        plan_hash: None,
        unchecked_tasks: 0,
        spec_modified: false,
        findings: vec![],
        token_usage: None,
    }
}

#[test]
fn redacts_bearer_and_prefixed_tokens() {
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints::default();
    let out = redactor
        .redact(
            raw_output("Authorization: Bearer sk-super-long-secret-123"),
            &hints,
        )
        .expect("redact");
    let value = out.gate_output.expect("output");
    assert!(!value.contains("sk-super-long-secret-123"));
    assert!(value.contains(REDACTION_TOKEN));
}

#[test]
fn redacts_explicit_secret_values_and_multiline_fragments() {
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints {
        required_secret_names: vec!["MY_SECRET".into()],
        secret_values: vec!["line1-secret\nline2-secret".into()],
    };
    let out = redactor
        .redact(
            raw_output("line1-secret / line2-secret / MY_SECRET=abc"),
            &hints,
        )
        .expect("redact");
    let gate = out.gate_output.expect("gate");
    assert!(!gate.contains("line1-secret"));
    assert!(!gate.contains("line2-secret"));
    assert!(!gate.contains("abc"));
}

#[test]
fn redacts_every_assignment_when_same_key_appears_multiple_times_on_line() {
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints {
        required_secret_names: vec!["API_TOKEN".into()],
        secret_values: vec![],
    };
    let out = redactor
        .redact(
            raw_output(
                "API_TOKEN=first API_TOKEN='second' api_token:\"third\" non_secret=ok API_TOKEN = fourth",
            ),
            &hints,
        )
        .expect("redact");
    let gate = out.gate_output.expect("gate");

    assert!(!gate.contains("first"));
    assert!(!gate.contains("second"));
    assert!(!gate.contains("third"));
    assert!(!gate.contains("fourth"));
    assert!(gate.contains("non_secret=ok"));
}

#[test]
fn redacts_assignment_variants_with_colon_and_quotes() {
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints {
        required_secret_names: vec!["my-secret".into()],
        secret_values: vec![],
    };
    let out = redactor
        .redact(
            raw_output("my-secret:abc my-secret : 'quoted' my-secret:\"double\""),
            &hints,
        )
        .expect("redact");
    let gate = out.gate_output.expect("gate");

    assert!(!gate.contains("abc"));
    assert!(!gate.contains("quoted"));
    assert!(!gate.contains("double"));
}

#[test]
fn leaves_non_sensitive_assignments_intact() {
    let redactor = DefaultOutputRedactor::default();
    let out = redactor
        .redact(
            raw_output("project_id=alpha region:us-central"),
            &RedactionHints::default(),
        )
        .expect("redact");
    let gate = out.gate_output.expect("gate");
    assert!(gate.contains("project_id=alpha"));
    assert!(gate.contains("region:us-central"));
}

#[test]
fn leak_detection_flags_remaining_secret() {
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints {
        required_secret_names: vec![],
        secret_values: vec!["secret-value".into()],
    };
    let output = PersistableOutput {
        outcome: Outcome::Success,
        signal: None,
        exit_code: None,
        duration_secs: FiniteF64::try_new(1.0).expect("finite"),
        gate_output: Some("still has secret-value".into()),
        tail_output: None,
        stderr_tail: None,
        pushed: false,
        plan_hash: None,
        unchecked_tasks: 0,
        spec_modified: false,
        findings: vec![],
        token_usage: None,
    };
    assert!(redactor.has_known_secret_leak(&output, &hints));
}

#[test]
fn rejects_non_finite_duration() {
    let redactor = DefaultOutputRedactor::default();
    let err = redactor
        .redact(
            RawExecutionOutput {
                duration_secs: f64::NAN,
                ..raw_output("ok")
            },
            &RedactionHints::default(),
        )
        .expect_err("must fail");
    assert_eq!(err, RedactionError::InvalidDuration);
}

proptest! {
    #[test]
    fn redact_does_not_leave_explicit_secret_values(
        secret in "[A-Za-z0-9_-]{12,40}",
        key in "[A-Za-z_][A-Za-z0-9_-]{3,16}"
    ) {
        let redactor = DefaultOutputRedactor::default();
        let hints = RedactionHints {
            required_secret_names: vec![key.clone()],
            secret_values: vec![secret.clone()],
        };
        let payload = format!("{key}={secret} and {secret} and {key}:{secret}");
        let out = redactor.redact(raw_output(&payload), &hints).expect("redact");

        for channel in [out.gate_output, out.tail_output, out.stderr_tail] {
            let text = channel.expect("channel");
            prop_assert!(!text.contains(&secret));
        }
    }

    #[test]
    fn redact_preserves_unrelated_benign_markers(
        benign in "[a-z]{5,20}",
        secret in "[A-Za-z0-9_-]{12,40}"
    ) {
        let redactor = DefaultOutputRedactor::default();
        let hints = RedactionHints {
            required_secret_names: vec!["API_TOKEN".into()],
            secret_values: vec![secret.clone()],
        };
        let marker = format!("SAFE_MARKER_{benign}");
        let payload = format!("API_TOKEN={secret} {marker}");
        let out = redactor.redact(raw_output(&payload), &hints).expect("redact");
        let gate = out.gate_output.expect("gate");

        prop_assert!(gate.contains(&marker));
        prop_assert!(!gate.contains(&secret));
    }
}
