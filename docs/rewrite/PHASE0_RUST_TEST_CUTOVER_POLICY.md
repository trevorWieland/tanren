# Phase 0 Rust Test Cutover Policy

## Status

Hard cutover is active as of task `T06` (`019db58b-b215-7fe3-ba25-45ffbdb973b3`).
Legacy Rust behavior-test suite paths have been removed from active proof flow.

## Deprecated Paths (Removed From Active Use)

The following legacy behavior-proof path is retired:

- `scripts/proof/phase0/run.sh` legacy nextest behavior-witness matrix

The legacy command matrix formerly recorded in `docs/rewrite/PHASE0_PROOF_EVIDENCE_INDEX.md`
no longer treats `cargo nextest` test names as behavior owners.

## Allowed Post-Cutover Test Forms

1. Behavior ownership: cucumber feature scenarios under `tests/bdd/phase0/*.feature`
   executed via `crates/tanren-bdd-phase0`.
2. Stage-gate verification commands:
   - `just check-phase0-bdd-smoke`
   - `just check-phase0-bdd-wave-a`
   - `just check-phase0-stage-gates`
3. Rust `nextest` suites remain allowed only as support/regression tests and must
   not be the canonical owner of `BEH-*` behavior claims.

## Prohibited Forms

- Re-introducing behavior ownership through Rust test-function command paths.
- Treating `cargo nextest` function names as the source of truth for Phase 0
  behavior acceptance.
- Skip/ignore suppression for behavior-owning scenarios.
