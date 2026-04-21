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
fn redacts_encoded_variants_for_long_secrets_within_contract_bounds() {
    let redactor = DefaultOutputRedactor::default();
    let secret = format!("{}+/=", "s3cr3t".repeat(30));
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
        secret_values: vec![RedactionSecret::from(secret.as_str())],
    };
    let payload = format!("{percent}\n{base64_std}\n{base64_url}");
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

#[test]
fn redaction_hints_validate_count_bounds() {
    let hints = RedactionHints {
        required_secret_names: vec![],
        secret_values: vec![RedactionSecret::from("x"); MAX_REDACTION_HINT_SECRET_COUNT + 1],
    };
    let err = hints
        .validate_bounds()
        .expect_err("must reject over-count hints");
    assert!(matches!(
        err,
        RedactionHintBoundsError::TooManySecrets {
            max_count: MAX_REDACTION_HINT_SECRET_COUNT,
            actual_count
        } if actual_count == MAX_REDACTION_HINT_SECRET_COUNT + 1
    ));
}

#[test]
fn redaction_hints_validate_secret_size_bounds() {
    let hints = RedactionHints {
        required_secret_names: vec![],
        secret_values: vec![RedactionSecret::from(
            "x".repeat(MAX_REDACTION_HINT_SECRET_BYTES + 1),
        )],
    };
    let err = hints
        .validate_bounds()
        .expect_err("must reject oversized secret value");
    assert!(matches!(
        err,
        RedactionHintBoundsError::SecretTooLarge {
            index: 0,
            max_bytes: MAX_REDACTION_HINT_SECRET_BYTES,
            actual_bytes
        } if actual_bytes == MAX_REDACTION_HINT_SECRET_BYTES + 1
    ));
}

#[test]
fn redaction_hints_validate_total_size_bounds() {
    let hints = RedactionHints {
        required_secret_names: vec![],
        secret_values: vec![RedactionSecret::from("a".repeat(MAX_REDACTION_HINT_SECRET_BYTES)); 17],
    };
    let err = hints
        .validate_bounds()
        .expect_err("must reject total bytes overflow");
    assert!(matches!(
        err,
        RedactionHintBoundsError::TotalBytesExceeded {
            max_total_bytes: MAX_REDACTION_HINT_TOTAL_SECRET_BYTES,
            actual_total_bytes
        } if actual_total_bytes > MAX_REDACTION_HINT_TOTAL_SECRET_BYTES
    ));
}

proptest! {
    #[test]
    fn redaction_stress_handles_high_hint_cardinality(
        secrets in prop::collection::vec("[A-Za-z0-9_\\-]{4,32}", 1..=128),
        payload_noise in prop::collection::vec("[A-Za-z0-9_\\-]{0,16}", 0..=64)
    ) {
        let redactor = DefaultOutputRedactor::default();
        let hints = RedactionHints {
            required_secret_names: vec![],
            secret_values: secrets
                .iter()
                .map(|secret| RedactionSecret::from(secret.as_str()))
                .collect(),
        };
        prop_assert!(hints.validate_bounds().is_ok());

        let mut payload = secrets.join(" ");
        if !payload_noise.is_empty() {
            payload.push(' ');
            payload.push_str(&payload_noise.join(" "));
        }
        let out = redactor.redact(raw_output(&payload), &hints).expect("redact");
        let joined = format!(
            "{} {} {}",
            out.gate_output.unwrap_or_default(),
            out.tail_output.unwrap_or_default(),
            out.stderr_tail.unwrap_or_default(),
        );

        for secret in &secrets {
            prop_assert!(!joined.contains(secret));
        }
    }

    #[test]
    fn redaction_stress_handles_boundary_hint_sizes(
        payload in ".{0,24000}",
        secrets in prop::collection::vec("[A-Za-z0-9_\\-]{1,256}", 0..=256)
    ) {
        let redactor = DefaultOutputRedactor::default();
        let hints = RedactionHints {
            required_secret_names: vec![],
            secret_values: secrets
                .iter()
                .map(|secret| RedactionSecret::from(secret.as_str()))
                .collect(),
        };
        prop_assert!(hints.validate_bounds().is_ok());
        let outcome = redactor
            .redact_with_audit(raw_output(&payload), &hints)
            .expect("redact");

        for channel in [
            outcome.output.gate_output.as_deref(),
            outcome.output.tail_output.as_deref(),
            outcome.output.stderr_tail.as_deref(),
        ]
        .iter()
        .flatten()
        {
            prop_assert!(
                channel.len() <= default_redaction_policy().max_persistable_channel_bytes()
                    + TRUNCATION_MARKER.len()
                    + 1
            );
        }
    }
}
