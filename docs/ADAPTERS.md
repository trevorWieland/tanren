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

Capability metadata is immutable in the adapter contract and exposed through
`HarnessAdapter::capabilities()`.
Preflight checks read adapter-declared data before execution side effects.

`HarnessRequirements` defines dispatch requirements and supports explicit
least-privilege bounds:

- capability minimums (for required features)
- privilege maximums (for sandbox/approval ceilings)
- dual approval ordering:
  - minimum approval uses strictness (`never < on_escalation < on_demand`)
  - maximum approval uses privilege risk (`on_demand < on_escalation < never`)
- validated bound types (`SandboxModeBounds`, `ApprovalModeBounds`) plus
  builder-based construction prevent invalid range objects at creation time.

`ensure_admissible` performs preflight checks and returns typed
`CompatibilityDenialKind` before side effects.

Approval denials are now represented only by reachable states:
`approval_mode_below_minimum` and `approval_mode_exceeds_maximum`.

### Execution Contract

`HarnessExecutionRequest` is the normalized request shape.

- `required_secret_names` is strongly typed (`SecretName`).
- explicit redaction hint values are hard-bounded:
  - max secret count: `256`
  - max per-secret bytes: `4096`
  - max total bytes: `65536`
- redaction secret values are in-memory only (`RedactionSecret`) and excluded
  from serialized payloads.

`execute_with_contract` is the required contract wrapper for persistence-bound
execution:

1. capability preflight
2. sealed adapter invocation (contract-only call token)
3. contract-owned redaction (not caller-injected)
4. single-pass redaction audit verdict (known-secret + residual policy)
5. metadata sanitization and allowlist validation
   (`provider_run_id`, `provider_code`, `provider_kind`, fail-closed)
   with typed `ProviderRunId` / `ProviderIdentifier` parsing
6. persistable output release

Adapters must not bypass this wrapper when producing persistable output.

### Failure Contract

`HarnessFailureClass` is the stable taxonomy consumed by orchestrator retry
policy (`to_domain_error_class`).

Adapters now return provider-native failures (`ProviderFailure`) and the
contract wrapper is the only normalization boundary to `HarnessFailure`.
`HarnessFailure` construction is crate-internal; callers cannot mint terminal
failures directly.

`ProviderFailureContext` supports typed/normalized classification:

- mandatory typed adapter code (`ProviderFailureCode`) for terminal failures
- terminal typed code is strict and has no `unknown` variant
- deterministic typed mapping only for semantic normalization
- optional audit-only fallback utility (`classify_provider_failure_for_audit`)
  is carried by `AuditProviderFailureContext`/`AuditProviderFailureCode`.

### Redaction Contract

Redaction runs before persistence on all channels:

- `gate_output`
- `tail_output`
- `stderr_tail`

Policy behavior:

- sensitive assignment redaction by normalized key
- JSON/YAML-style quoted-key assignment redaction
- bearer token redaction
- case-insensitive prefixed token redaction
- explicit secret value redaction (including multiline fragments)
- context-aware short multiline-fragment redaction (bounded rules)
- encoded secret variant redaction (URL/base64 forms derived from known hints)
- bounded per-channel persistence with deterministic truncation marker
- validated immutable policy schema (`RedactionPolicy::try_new` / builder)
- precompiled prefix trie and multi-pattern secret matcher (Aho-Corasick style)
  for scalable hint/prefix cardinality and per-call reuse

`RedactionHints` is secret-safe:

- debug output redacts secret contents
- secret material uses secrecy-backed wrappers (no plain-text debug dumps)

## Conformance Contract

Lane 1.2+ adapter crates must reuse `tanren-runtime` conformance helpers:

- `assert_capability_denial_is_preflight`
- `assert_redaction_before_persistence`
- `assert_failure_classification`
- `assert_provider_metadata_fail_closed`
- `assert_terminal_typed_code_mapping`

Required adapter evidence:

1. capability denial is preflight-only
2. redaction completes before persistence
3. failure mapping is stable across provider-native payloads
4. provider metadata sanitization is fail-closed
5. terminal typed-code semantics remain deterministic
6. contract event checks enforce exact cardinality and ordering (no duplicates)

## Implementation Notes

- `OutputRedactor` remains a runtime abstraction, but persistence-bound policy
  enforcement is sealed inside `execute_with_contract`.
- `HarnessAdapter::execute` requires an unconstructable contract token so
  callers cannot directly invoke adapter execution from outside the contract.
- `HarnessAdapterRegistry` enables trait-object adapter registration/lookup for
  runtime registry patterns.
- provider metadata is sanitized and validated under a strict fail-closed policy
  before exposure/persistence.
- adapter failure payload sanitization emits a dedicated
  `FailurePathRedactionLeakDetected` contract error on residual leaks.
- default redaction policy/patterns are sourced from a versioned dataset.
- test-only customization for contract internals is crate-scoped.
- `HarnessExecutionEvent` is source-tagged (`contract` vs `adapter`) and adapter
  emissions are proxied as adapter-source events so conformance ordering cannot
  be polluted by adapter-generated contract-like events.
