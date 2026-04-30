---
schema: tanren.implementation_readiness_view.v0
source: readiness.json
status: current
owner_command: assess-implementation
updated_at: 2026-04-29
---

# Implementation Readiness

This file is the human-readable projection of current behavior implementation
readiness. The machine-readable source is `readiness.json`.

The current temporary readiness mechanism is `scripts/behavior-readiness.sh`.
It runs read-only static analysis over accepted behavior files whose
verification status is below `asserted`, saves interruptible per-behavior
reports under `artifacts/behavior/readiness/runs/`, and writes the aggregate
summary to `docs/implementation/readiness.json`.

## Status

No complete aggregate readiness report has been committed yet in the new
implementation ownership structure.

## Interpretation

- `already_implemented` means code appears to support the behavior end to end,
  though behavior proof may still be missing.
- `close_needs_work` means a clear implementation surface exists and bounded
  gaps remain.
- `partial_foundation` means adjacent primitives exist, but meaningful product
  behavior remains.
- `not_started` means little or no relevant implementation exists.
- `unclear` means source signals are contradictory or too thin.

Roadmap synthesis should consume this projection as current-state implementation
assessment, not
as product intent.
