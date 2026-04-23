#!/usr/bin/env bash
set -euo pipefail

cat >&2 <<'MSG'
scripts/proof/phase0/run.sh is intentionally retired during the Phase 0 Rust hard cutover.

Legacy Rust behavior-test suite command paths were removed in T06 and are no
longer accepted as behavior ownership evidence.

Use post-cutover commands instead:
  just check-phase0-scenario-stage
  just check-phase0-bdd-smoke
  just check-phase0-bdd-wave-a
  just check-phase0-bdd-wave-b
  just check-phase0-bdd-wave-c
  just check-phase0-mutation-stage
  just check-phase0-coverage-stage
  just check-phase0-stage-gates

Policy and allowed forms:
  docs/rewrite/PHASE0_RUST_TEST_CUTOVER_POLICY.md
MSG

exit 2
