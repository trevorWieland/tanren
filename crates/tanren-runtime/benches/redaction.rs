use std::fmt::Write as _;

use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use tanren_domain::{FiniteF64, Outcome};
use tanren_runtime::{
    DefaultOutputRedactor, OutputRedactor, RawExecutionOutput, RedactionHints, RedactionPolicy,
    RedactionSecret, SecretName,
};

fn base_output(
    gate_output: String,
    tail_output: String,
    stderr_tail: String,
) -> RawExecutionOutput {
    RawExecutionOutput {
        outcome: Outcome::Success,
        signal: None,
        exit_code: Some(0),
        duration_secs: FiniteF64::try_new(1.0).expect("finite"),
        gate_output: Some(gate_output),
        tail_output: Some(tail_output),
        stderr_tail: Some(stderr_tail),
        pushed: false,
        plan_hash: None,
        unchecked_tasks: 0,
        spec_modified: false,
        findings: Vec::new(),
        token_usage: None,
    }
}

fn bench_large_output(c: &mut Criterion) {
    let redactor = DefaultOutputRedactor::default();
    let hints = RedactionHints {
        required_secret_names: vec![SecretName::try_new("API_TOKEN").expect("secret key")],
        secret_values: vec![RedactionSecret::from("sk-live-secret-value-123")],
    };

    let repeated =
        "Authorization: Bearer sk-live-secret-value-123 API_TOKEN=sk-live-secret-value-123 SAFE\n"
            .repeat(8_000);
    let output = base_output(repeated.clone(), repeated.clone(), repeated);

    c.bench_function("redaction/large_output", |b| {
        b.iter_batched(
            || output.clone(),
            |raw| {
                let result = redactor.redact_with_audit(raw, &hints);
                let _ = black_box(result);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_many_secrets(c: &mut Criterion) {
    let redactor = DefaultOutputRedactor::default();
    let secret_values = (0..200)
        .map(|idx| RedactionSecret::from(format!("secret-{idx:04}-value-abcdefghijkl")))
        .collect::<Vec<_>>();
    let required_secret_names = (0..50)
        .map(|idx| SecretName::try_new(format!("TOKEN_{idx:02}")).expect("secret name"))
        .collect::<Vec<_>>();
    let hints = RedactionHints {
        required_secret_names,
        secret_values,
    };

    let mut payload = String::new();
    for idx in 0..200 {
        let _ = write!(
            payload,
            "TOKEN_{:02}=secret-{idx:04}-value-abcdefghijkl ",
            idx % 50
        );
    }
    let output = base_output(payload.clone(), payload.clone(), payload);

    c.bench_function("redaction/many_secret_hints", |b| {
        b.iter_batched(
            || output.clone(),
            |raw| {
                let result = redactor.redact_with_audit(raw, &hints);
                let _ = black_box(result);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_many_policy_prefixes(c: &mut Criterion) {
    let mut group = c.benchmark_group("redaction/prefix_density");
    for prefix_count in [10_usize, 50, 200] {
        let policy = RedactionPolicy {
            min_token_len: 8,
            min_secret_fragment_len: 6,
            sensitive_key_names: vec!["api_key".to_owned(), "token".to_owned()],
            token_prefixes: (0..prefix_count)
                .map(|idx| format!("p{idx:03}_"))
                .collect::<Vec<_>>(),
            max_persistable_channel_bytes: 256 * 1024,
        };
        let redactor = DefaultOutputRedactor::new(policy);
        let mut payload = String::new();
        for idx in 0..800 {
            let _ = write!(payload, "p{:03}_TOKEN_VALUE_{idx:04} ", idx % prefix_count);
        }
        let output = base_output(payload.clone(), payload.clone(), payload);

        group.bench_with_input(
            BenchmarkId::from_parameter(prefix_count),
            &output,
            |b, raw| {
                b.iter_batched(
                    || raw.clone(),
                    |input| {
                        let result = redactor.redact_with_audit(input, &RedactionHints::default());
                        let _ = black_box(result);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_large_output(c);
    bench_many_secrets(c);
    bench_many_policy_prefixes(c);
}

criterion_group!(redaction_benches, benches);
criterion_main!(redaction_benches);
