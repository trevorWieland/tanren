# Lane 1.2 — Initial Harness Adapters

## Status

Planned for Phase 1.

## Purpose

Implement the first set of harness adapters on top of the Lane 1.1
contract and prove that harness choice does not change domain-level
execution semantics.

Behavioral baseline: [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md)
Feature 1, Feature 3, and Feature 6.

## Scope

This lane ships:

- three production harness adapters: Claude Code, Codex, and OpenCode
- capability reporting for each shipped adapter
- typed failure mapping from harness-native errors into Tanren classes
- shared redaction contract usage via `OutputRedactor`
- conformance and parity evidence across the shipped harnesses

Lane 1.2 adapters must consume the Lane 1.1 runtime contract APIs in
`crates/tanren-runtime`, especially:

- `execute_with_contract` for preflight + redaction enforcement
- `DefaultOutputRedactor` / `RedactionHints` for output safety
- `assert_capability_denial_is_preflight`
- `assert_redaction_before_persistence`
- `assert_failure_classification`

## Behavioral requirements

1. Equivalent dispatches produce equivalent domain outcomes across
   all three shipped harnesses.
2. Capability mismatches produce typed denials, not partial execution.
3. Failure classes are stable and operator-readable across adapters.
4. Redaction requirements from Lane 1.1 remain enforced in every adapter.

## Dependencies

- [LANE-1.1-HARNESS.md](LANE-1.1-HARNESS.md)

## Out of scope

- environment lease contract and adapters
- worker retry/recovery orchestration
- adding a fourth harness adapter in this lane
