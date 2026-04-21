# Redaction Benchmark Baseline Scenarios

This file defines the fixed benchmark scenario set for `benches/redaction.rs`.
It is the baseline artifact used for regression comparisons across runs.
`redaction-thresholds.json` is the machine-enforced CI budget for these scenarios.

## Scenarios

1. `redaction/large_output`
- ~8k repeated lines with bearer + assignment secrets across all channels.
- Validates throughput and allocation behavior under large payload redaction.

2. `redaction/many_secret_hints`
- 200 explicit secret values + 50 required secret keys.
- Validates matcher scale when hint lists are large.

3. `redaction/prefix_density/{10,50,200}`
- Varying token-prefix list sizes with dense prefixed token payloads.
- Validates scanner scaling as policy prefix set grows.

## Usage

Run locally:

```bash
cargo bench -p tanren-runtime --bench redaction
uv run python scripts/check_redaction_perf.py
```

The checker reads Criterion outputs under `target/criterion` and fails when any
scenario mean exceeds its threshold budget.
