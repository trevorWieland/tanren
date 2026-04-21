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
Lane 1.1 acceptance explicitly excludes shipping concrete harness adapter
implementations; those are mandatory Lane 1.2 deliverables.

## Scope

This lane ships (in `crates/tanren-runtime`):

- typed capability model and preflight compatibility checks
- normalized execution request/result surfaces
- normalized typed harness failure taxonomy + mapping to domain retry class
- redaction-before-persistence runtime contract
- reusable adapter conformance test helpers for Lane 1.2

This lane does **not** ship concrete provider adapters.
This lane also does **not** claim cross-harness parity proof based on adapter
execution; parity proof is accepted only in Lane 1.2.

## Contract Surfaces

### Capability Contract

- `HarnessCapabilities` advertises adapter support for:
  - output streaming class
  - tool-use support
  - patch-apply support
  - session-resume support
  - sandbox mode
  - approval mode
- `HarnessRequirements` describes what a dispatch needs and is constructed via
  validated builder APIs.
- Sandbox and approval requirements use validated bound types
  (`SandboxModeBounds`, `ApprovalModeBounds`) for minimum capability and maximum
  privilege constraints.
- Approval bounds are dual-axis:
  - minimum approval uses strictness (`never < on_escalation < on_demand`)
  - maximum approval uses privilege risk (`on_demand < on_escalation < never`)
- Approval denials are modeled with reachable states only:
  `approval_mode_below_minimum` and `approval_mode_exceeds_maximum`.
- `ensure_admissible` performs preflight checks and returns typed
  `CompatibilityDenial` when unsupported.

### Execution Contract

- `HarnessExecutionRequest` is the provider-agnostic input shape.
- Secret material in `HarnessExecutionRequest` is carried as secrecy-backed
  values and never serialized.
- `HarnessAdapter::execute` returns `ExecutionSignal` with raw output and
  provider-native failure payloads (`ProviderFailure`).
- `execute_with_contract` enforces:
  1. preflight capability check
  2. sealed adapter execution (contract-only call token)
  3. redaction hints derived from request data (not caller-provided)
  4. contract-owned redaction + single-pass leak audit to `PersistableOutput`
  5. fail-closed metadata sanitization for `provider_run_id`,
     `provider_code`, and `provider_kind`
  6. source-tagged event emission (`contract` vs `adapter`) for conformance-safe
     ordering assertions

### Failure Contract

- `HarnessFailureClass` is the stable failure taxonomy.
- `HarnessFailure` is constructor-normalized with invariant-safe class/code
  states; conflicting class/typed-code combinations are rejected at
  deserialization boundaries.
- `ProviderFailureContext.typed_code` is mandatory for terminal adapter
  failures and uses terminal-only `ProviderFailureCode` (no `unknown` variant).
- `classify_provider_failure` maps terminal provider failures deterministically
  from typed adapter code only.
- `classify_provider_failure_for_audit` remains available via
  `AuditProviderFailureContext`/`AuditProviderFailureCode` as an explicit
  telemetry utility for bounded fallback heuristics.
- `HarnessFailureClass::to_domain_error_class` maps to
  `tanren_domain::ErrorClass` for retry policy consistency.

### Redaction Contract

- `OutputRedactor` defines redaction behavior; persistence-bound execution uses
  the contract-owned default redactor.
- `DefaultOutputRedactor` + `RedactionPolicy` provide the shared baseline.
- Default redaction policy data is sourced from a versioned dataset.
- `RedactionHints` is an internal capture-time representation derived from
  request secrets by the contract wrapper.
- Redaction matcher coverage includes context-aware short multiline fragments
  and encoded secret variants (URL/base64 forms) derived from known hints.
- Redaction is applied to all persistable output channels:
  `gate_output`, `tail_output`, `stderr_tail`.

## Behavioral Requirements

1. Harness choice must not change domain-level terminal semantics.
2. Unsupported capability requirements are denied before adapter side effects.
3. Raw provider failures map to typed contract classes.
4. Persisted output is redacted before durable storage.
5. Conformance is executable through reusable tests, not prose-only.
6. Verification for this lane is executed with `just ci` from repo root (`make`-based checks are legacy and non-authoritative for acceptance).
7. Lane 1.1 audits must score adapter implementation completeness as out-of-scope and defer that criterion to Lane 1.2.

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
- `assert_provider_metadata_fail_closed`
- `assert_terminal_typed_code_mapping`

Each adapter must prove:

1. capability denial happens before side effects
2. redaction is complete before persistence
3. provider-specific failures normalize to stable failure classes
4. unsafe provider metadata is rejected fail-closed
5. typed terminal failure code mappings remain stable

Lane 1.2 acceptance also requires parity scenarios for
[../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md) Feature 1, Feature 3, and
Feature 6 across all shipped harness adapters.

## Dependencies

- Phase 0 foundation complete (`0.1` through `0.5`)

## Out of Scope

- provider CLI command wiring
- environment lease contract/implementations
- worker retry orchestration and crash recovery runtime
