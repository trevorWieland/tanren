use super::*;

#[test]
fn redacts_contextual_short_multiline_secret_fragments() {
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints {
        required_secret_names: vec![SecretName::try_new("api_key").expect("secret key")],
        secret_values: vec![RedactionSecret::from("very-long-secret\nx9-")],
    };
    let out = redactor
        .redact(raw_output("api_key=x9- marker=ok prefixx9-suffix"), &hints)
        .expect("redact");
    let gate = out.gate_output.expect("gate");
    assert!(!gate.contains("api_key=x9-"), "{gate}");
    assert!(gate.contains("prefixx9-suffix"), "{gate}");
}

#[test]
fn redacts_url_and_base64_encoded_secret_variants() {
    let redactor = DefaultOutputRedactor::default();
    let secret = "s3cr3t+/=";
    let percent = secret_matcher::percent_encode(secret.as_bytes());
    let base64_std = secret_matcher::base64_encode(
        secret.as_bytes(),
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
        true,
    );
    let base64_url = secret_matcher::base64_encode(
        secret.as_bytes(),
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
        false,
    );
    let hints = RedactionHints {
        required_secret_names: vec![],
        secret_values: vec![RedactionSecret::from(secret)],
    };
    let payload = format!("{percent} {base64_std} {base64_url}");
    let out = redactor
        .redact(raw_output(&payload), &hints)
        .expect("redact");
    let gate = out.gate_output.expect("gate");
    assert!(!gate.contains(&percent), "{gate}");
    assert!(!gate.contains(&base64_std), "{gate}");
    assert!(!gate.contains(&base64_url), "{gate}");
}

#[test]
fn redact_with_audit_returns_single_verdict_for_channels() {
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints {
        required_secret_names: vec![SecretName::try_new("API_TOKEN").expect("secret key")],
        secret_values: vec![RedactionSecret::from("line1-secret\nline2-secret")],
    };
    let result = redactor
        .redact_with_audit(
            RawExecutionOutput {
                outcome: Outcome::Success,
                signal: None,
                exit_code: Some(0),
                duration_secs: FiniteF64::try_new(1.2).expect("finite"),
                gate_output: Some("API_TOKEN=abc SAFE_MARKER".into()),
                tail_output: Some("line1-secret".into()),
                stderr_tail: Some("line2-secret".into()),
                pushed: false,
                plan_hash: None,
                unchecked_tasks: 0,
                spec_modified: false,
                findings: vec![],
                token_usage: None,
            },
            &hints,
        )
        .expect("redact");
    assert!(!result.audit.has_any_leak());
    let gate = result.output.gate_output.expect("gate");
    assert!(gate.contains("SAFE_MARKER"), "{gate}");
}
