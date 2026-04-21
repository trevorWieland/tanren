use super::*;

#[test]
fn redaction_policy_rejects_invalid_thresholds() {
    let err = RedactionPolicy::try_new(4, 3, vec!["api_key".into()], vec!["sk-".into()], 2048)
        .expect_err("min token len below secure floor must fail");
    assert!(matches!(
        err,
        RedactionPolicyError::MinTokenLenTooSmall { .. }
    ));

    let err = RedactionPolicy::try_new(10, 11, vec!["api_key".into()], vec!["sk-".into()], 2048)
        .expect_err("secret fragment len above token len must fail");
    assert!(matches!(
        err,
        RedactionPolicyError::MinSecretFragmentLenOutOfRange { .. }
    ));
}

#[test]
fn redaction_policy_normalizes_and_validates_duplicates() {
    let policy = RedactionPolicy::try_new(
        10,
        4,
        vec![" API_KEY ".into(), "client_secret".into()],
        vec![" SK- ".into(), "ghp_".into()],
        2048,
    )
    .expect("valid policy");

    let keys = policy
        .sensitive_key_names()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["api_key", "client_secret"]);

    let prefixes = policy
        .token_prefixes()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(prefixes, vec!["sk-", "ghp_"]);

    let duplicate = RedactionPolicy::try_new(
        10,
        4,
        vec!["api_key".into(), "API_KEY".into()],
        vec!["sk-".into()],
        2048,
    )
    .expect_err("duplicate key names must be rejected");
    assert!(matches!(
        duplicate,
        RedactionPolicyError::DuplicateSensitiveKeyName { .. }
    ));
}

#[test]
fn redaction_policy_deserialization_fails_for_invalid_payloads() {
    let payload = serde_json::json!({
        "min_token_len": 2,
        "min_secret_fragment_len": 1,
        "sensitive_key_names": ["api_key"],
        "token_prefixes": ["sk-"],
        "max_persistable_channel_bytes": 2048
    });
    let err = serde_json::from_value::<RedactionPolicy>(payload).expect_err("must reject");
    assert!(
        err.to_string().contains("min_token_len"),
        "unexpected deserialization error: {err}"
    );
}

#[test]
fn redaction_matchers_compile_to_bounded_state_machines() {
    let prefixes = (0..200)
        .map(|idx| format!("p{idx:03}_"))
        .collect::<Vec<_>>();
    let prefix_matcher = scanner::CompiledTokenPrefixMatcher::new(&prefixes);
    let prefix_stats = prefix_matcher.stats();
    assert_eq!(prefix_stats.prefixes, 200);
    assert!(prefix_stats.nodes <= 1001);
    assert!(prefix_stats.transitions <= 1000);

    let hints = RedactionHints {
        required_secret_names: Vec::new(),
        secret_values: (0..120)
            .map(|idx| RedactionSecret::from(format!("secret-{idx:03}-abcdefghijklmnop")))
            .collect(),
    };
    let matcher = CompiledSecretMatcher::from_hints(&hints, 4);
    let matcher_stats = matcher.stats();
    assert!(matcher_stats.patterns >= 120);
    assert!(matcher_stats.states > 1);
    assert!(matcher_stats.transitions >= matcher_stats.patterns);
}
