# Phase 0 Manual Walkthrough Proof

## Objective

Provide a canonical, operator-readable proof artifact set for Feature `8.1`
(the manual 7-step self-hosting loop from `shape-spec` through `walk-spec`).

This pack is intentionally narrative + machine-readable so a technically
literate teammate can validate Phase 0 behavior without code spelunking.

---

## Canonical Sample Bundle

- `docs/rewrite/proof-samples/phase0-manual-walkthrough-2026-04-20/`

Bundle contents:

- `README.md` — sequence overview and command provenance
- `summary.json` — machine-readable pass/fail summary
- `start-task-list.json` — pre-loop state snapshot
- `final-task-list.json` — post-loop state snapshot
- `phase-events.jsonl` — authoritative typed event trail
- `steps/` — per-step command, stdout, stderr artifacts

---

## 7-Step Sequence Captured

1. `shape-spec`: set spec title, create first task, add demo step.
2. resolve task context: list tasks for the spec.
3. `do-task`: start + complete the selected task.
4. `audit-task`: add typed audit finding attached to the task.
5. `run-demo`: append demo result evidence.
6. `audit-spec`: report typed phase outcome.
7. `walk-spec`: report typed phase outcome and confirm coherent final task view.

---

## Why This Proves Feature 8.1

The sample demonstrates all required BDD proof contents in one place:

- explicit start state
- every phase invocation from `shape-spec` to `walk-spec`
- typed state transitions and event trail (`phase-events.jsonl`)
- coherent task/finding/progress outputs across the full loop

---

## Regeneration Path

A fresh runtime pack can be generated with:

```bash
scripts/proof/phase0/run.sh
```

The runtime equivalent lives at:

- `artifacts/phase0-proof/<timestamp>/manual-walkthrough/`

Use `docs/rewrite/PHASE0_PROOF_RUNBOOK.md` for full rerun + verification flow.
