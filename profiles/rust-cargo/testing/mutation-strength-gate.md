---
kind: standard
name: mutation-strength-gate
category: testing
importance: high
applies_to:
  - "**/*test*"
  - "**/*spec*"
  - "tests/**"
applies_to_languages:
  - rust
applies_to_domains:
  - testing
---

# Mutation Strength Gate (cargo-mutants)

`cargo-mutants` is required to validate scenario effectiveness against product
code, not primarily against the BDD harness.

```bash
# Example command shape
cargo mutants --workspace
```

**Rules:**
- Scheduled CI must run `cargo-mutants` against product crates and binaries
  discovered from the workspace; PR CI may omit mutation when behavior and
  coverage gates run on every change
- The BDD runner, testkit, xtask, and generated artifacts are excluded from
  product mutation
- The mutation test command is the full behavior suite through `tanren-bdd`
- Mutation may be split by automatically discovered package shards, but shard
  selection must come from Cargo metadata, not hand-maintained phase, wave, or
  source-file lists
- Surviving mutants require either new/improved scenarios or explicit rationale
- Missed mutants, timeouts, baseline failures, and untriaged unviable mutants
  fail the scheduled mutation job and must raise a tracked issue
- Mutation findings should reference affected behavior IDs

**Why:** Mutation testing catches weak scenarios that pass without actually protecting behavior.
