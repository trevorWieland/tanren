# Phase 0 Proof Evidence Index

Freshness timestamp: `2026-04-23T00:00:00Z`
BDD source: `docs/rewrite/PHASE0_PROOF_BDD.md`
Cutover policy: `docs/rewrite/PHASE0_RUST_TEST_CUTOVER_POLICY.md`

## Cutover Note

Hard cutover removed the legacy Rust behavior-test command matrix that treated
`cargo nextest` test names as behavior owners.

Behavior ownership now lives in BDD feature files (`tests/bdd/phase0/*.feature`).
`nextest` remains support/regression coverage and is not the behavior source of
truth.

## Scenario Matrix (Post-Cutover)

| Scenario | Behavior owner | Current proof path | Status |
|---|---|---|---|
| 1.1 | `@BEH-P0-101` | `tests/bdd/phase0/feature-1-typed-control-plane-state.feature` | active |
| 1.2 | `@BEH-P0-102` | `tests/bdd/phase0/feature-1-typed-control-plane-state.feature` | active |
| 2.1 | `@BEH-P0-201` | `tests/bdd/phase0/feature-2-event-history.feature` | active |
| 2.2 | `@BEH-P0-202` | `tests/bdd/phase0/feature-2-event-history.feature` | active |
| 2.3 | `@BEH-P0-203` | `tests/bdd/phase0/feature-2-event-history.feature` | active |
| 3.1 | `@BEH-P0-301` | `tests/bdd/phase0/feature-3-contract-derived-interface.feature` | active |
| 3.2 | `@BEH-P0-302` | `tests/bdd/phase0/feature-3-contract-derived-interface.feature` | active |
| 4.1 | `@BEH-P0-401` | `tests/bdd/phase0/feature-4-methodology-boundary.feature` | active |
| 4.2 | `@BEH-P0-402` | `tests/bdd/phase0/feature-4-methodology-boundary.feature` | active |
| 5.1 | `@BEH-P0-501` | `tests/bdd/phase0/feature-5-task-completion-guards.feature` | active |
| 5.2 | `@BEH-P0-502` | `tests/bdd/phase0/feature-5-task-completion-guards.feature` | active |
| 6.1 | `@BEH-P0-601` | `tests/bdd/phase0/feature-6-tool-surface-contract.feature` | active |
| 6.2 | `@BEH-P0-602` | `tests/bdd/phase0/feature-6-tool-surface-contract.feature` | active |
| 6.3 | `@BEH-P0-603` | `tests/bdd/phase0/feature-6-tool-surface-contract.feature` | active |
| 7.1 | `@BEH-P0-701` | `tests/bdd/phase0/feature-7-installer-determinism.feature` | active |
| 7.2 | `@BEH-P0-702` | `tests/bdd/phase0/feature-7-installer-determinism.feature` | active |
| 7.3 | `@BEH-P0-703` | `tests/bdd/phase0/feature-7-installer-determinism.feature` | active |
| 8.1 | `@BEH-P0-801` | `tests/bdd/phase0/feature-8-manual-methodology-walkthrough.feature` | active |

## Current Commands

- `just check-phase0-bdd-smoke`
- `just check-phase0-bdd-wave-a`
- `just check-phase0-bdd-wave-b`
- `just check-phase0-bdd-wave-c`
- `just check-phase0-mutation-stage`
- `just check-phase0-coverage-stage`
- `just check-phase0-stage-gates`

## Supplemental Packs

- `artifacts/phase0-proof/<timestamp>/auth-replay/summary.json`
- `artifacts/phase0-proof/<timestamp>/replay-pack/verdicts/equivalence.json`
- `artifacts/phase0-proof/<timestamp>/replay-pack/verdicts/rollback.json`
- `artifacts/phase0-proof/<timestamp>/manual-walkthrough/summary.json`
- `docs/rewrite/proof-samples/phase0-manual-walkthrough-2026-04-20/`
- `artifacts/phase0-mutation/staged/latest/triage.json`
- `artifacts/phase0-coverage/staged/latest/classification.json`
