# Phase 0 BDD Feature Conventions

This directory is the canonical home for Phase 0 behavior scenarios.

Hard cutover policy:
- `docs/rewrite/PHASE0_RUST_TEST_CUTOVER_POLICY.md`

Conventions:
- Feature files live under `tests/bdd/phase0/*.feature`.
- Filenames use `feature-<n>-<scope>.feature` for behavior waves, plus `smoke.feature`.
- Every behavior-owning scenario must include exactly one stable `@BEH-*` tag.
- Witness tags are explicit (`@positive` or `@falsification`) so each behavior can prove both directions.
- Skip tags (`@wip`, `@ignore`) are prohibited for behavior-owning scenarios.

Stage commands:
- `just check-phase0-bdd-smoke`
- `just check-phase0-bdd-wave-a`
- `just check-phase0-bdd-wave-b`
- `just check-phase0-bdd-wave-c`
- `just check-phase0-mutation-stage`
- `just check-phase0-coverage-stage`
- `just check-phase0-stage-gates`
