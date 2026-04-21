# Lane 1.1 — Harness Adapter Contract

## Status

Implemented on `rewrite/lane-1-1` (pending merge to foundation).

## Purpose

Define one canonical harness contract so dispatch semantics stay stable
across providers (Claude Code, Codex, OpenCode, future adapters).

Behavioral baseline: [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md)
Feature 1, Feature 3, and Feature 6.

Lane 1.1 establishes the canonical contract and executable conformance
checks required to validate those features. Cross-harness semantic
equivalence evidence is accepted in Lane 1.2, where concrete adapters exist.

## Scope

This lane ships (in `crates/tanren-runtime`):

- typed capability model and preflight compatibility checks
- normalized execution request/result surfaces
- normalized typed harness failure taxonomy + mapping to domain retry class
- redaction-before-persistence runtime contract
- reusable adapter conformance test helpers for Lane 1.2

This lane does **not** ship concrete provider adapters.

## Contract Surfaces

### Capability Contract

- `HarnessCapabilities` advertises adapter support for:
  - output streaming class
  - tool-use support
  - patch-apply support
  - session-resume support
  - sandbox mode
  - approval mode
- `HarnessRequirements` describes what a dispatch needs.
- Sandbox and approval requirements support both minimum capability and maximum
  privilege bounds.
- `ensure_admissible` performs preflight checks and returns typed
  `CompatibilityDenial` when unsupported.

### Execution Contract

- `HarnessExecutionRequest` is the provider-agnostic input shape.
- Secret material in `HarnessExecutionRequest` is carried as secrecy-backed
  values and never serialized.
- `HarnessAdapter::execute` returns `ExecutionSignal` with raw output.
- `execute_with_contract` enforces:
  1. preflight capability check
  2. adapter execution
  3. redaction hints derived from request data (not caller-provided)
  4. contract-owned redaction to `PersistableOutput`
  5. known-secret leak check before persistence

### Failure Contract

- `HarnessFailureClass` is the stable failure taxonomy.
- `classify_provider_failure` maps provider-native context into
  `HarnessFailureClass`, preferring typed adapter codes and structured
  context before text fallback heuristics.
- `HarnessFailureClass::to_domain_error_class` maps to
  `tanren_domain::ErrorClass` for retry policy consistency.

### Redaction Contract

- `OutputRedactor` defines redaction behavior; persistence-bound execution uses
  the contract-owned default redactor.
- `DefaultOutputRedactor` + `RedactionPolicy` provide the shared baseline.
- `RedactionHints` is an internal capture-time representation derived from
  request secrets by the contract wrapper.
- Redaction is applied to all persistable output channels:
  `gate_output`, `tail_output`, `stderr_tail`.

## Behavioral Requirements

1. Harness choice must not change domain-level terminal semantics.
2. Unsupported capability requirements are denied before adapter side effects.
3. Raw provider failures map to typed contract classes.
4. Persisted output is redacted before durable storage.
5. Conformance is executable through reusable tests, not prose-only.

## Redaction Minimum Coverage

The default policy must cover, at minimum:

1. common credential/token patterns
2. explicit secret values resolved for runtime injection
3. credential-file content style patterns (key/value forms)
4. multiline secret fragments

## Conformance Expectations

Lane 1.2 adapter crates must reuse `tanren-runtime` conformance helpers:

- `assert_capability_denial_is_preflight`
- `assert_redaction_before_persistence`
- `assert_failure_classification`

Each adapter must prove:

1. capability denial happens before side effects
2. redaction is complete before persistence
3. provider-specific failures normalize to stable failure classes

Lane 1.2 acceptance also requires parity scenarios for
[../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md) Feature 1, Feature 3, and
Feature 6 across all shipped harness adapters.

## Dependencies

- Phase 0 foundation complete (`0.1` through `0.5`)

## Out of Scope

- provider CLI command wiring
- environment lease contract/implementations
- worker retry orchestration and crash recovery runtime
