# Adapter Architecture (Rust Runtime Contract)

This document is the canonical runtime adapter reference for Lane 1.1+.

The canonical implementation lives in `crates/tanren-runtime` and defines the
provider-agnostic harness contract that all concrete harness adapters must use
(Claude, Codex, OpenCode, and future adapters).

## Contract Surface

### Capability Contract

`HarnessCapabilities` declares adapter support across:

- output streaming
- tool use
- patch apply level
- session resume level
- sandbox mode
- approval mode

`HarnessRequirements` defines dispatch requirements and now supports explicit
least-privilege bounds:

- capability minimums (for required features)
- privilege maximums (for sandbox/approval ceilings)

`ensure_admissible` performs preflight checks and returns typed
`CompatibilityDenialKind` before side effects.

### Execution Contract

`HarnessExecutionRequest` is the normalized request shape.

- `required_secret_names` is strongly typed (`SecretName`).
- redaction secret values are in-memory only (`RedactionSecret`) and excluded
  from serialized payloads.

`execute_with_contract` is the required contract wrapper for persistence-bound
execution:

1. capability preflight
2. adapter invocation
3. contract-owned redaction (not caller-injected)
4. known-secret leak check
5. persistable output release

Adapters must not bypass this wrapper when producing persistable output.

### Failure Contract

`HarnessFailureClass` is the stable taxonomy consumed by orchestrator retry
policy (`to_domain_error_class`).

`ProviderFailureContext` supports typed/normalized classification:

- typed adapter code first (`ProviderFailureCode`)
- normalized provider identifiers (`ProviderIdentifier`)
- deterministic signal/exit-code mapping
- boundary-aware text fallback last

### Redaction Contract

Redaction runs before persistence on all channels:

- `gate_output`
- `tail_output`
- `stderr_tail`

Policy behavior:

- sensitive assignment redaction by normalized key
- bearer token redaction
- case-insensitive prefixed token redaction
- explicit secret value redaction (including multiline fragments)
- bounded per-channel persistence with deterministic truncation marker

`RedactionHints` is secret-safe:

- debug output redacts secret contents
- secret material uses secrecy-backed wrappers (no plain-text debug dumps)

## Conformance Contract

Lane 1.2+ adapter crates must reuse `tanren-runtime` conformance helpers:

- `assert_capability_denial_is_preflight`
- `assert_redaction_before_persistence`
- `assert_failure_classification`

Required adapter evidence:

1. capability denial is preflight-only
2. redaction completes before persistence
3. failure mapping is stable across provider-native payloads

## Implementation Notes

- `OutputRedactor` remains a runtime abstraction, but persistence-bound policy
  enforcement is sealed inside `execute_with_contract`.
- test-only customization for contract internals is crate-scoped.
- contract event ordering is observable through `HarnessObserver` for
  deterministic tests and telemetry.
