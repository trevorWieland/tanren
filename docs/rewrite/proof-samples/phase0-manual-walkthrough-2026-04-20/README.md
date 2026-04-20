# Phase 0 Manual Walkthrough Sample (2026-04-20)

This committed bundle captures one full manual self-hosting loop for
Feature `8.1`, aligned to the 7-step sequence in
`docs/rewrite/METHODOLOGY_BOUNDARY.md`.

## Capture Source

- Runtime pack: `artifacts/phase0-proof/20260420T224112Z/manual-walkthrough/`
- Capture command: `scripts/proof/phase0/run.sh`

## Included Artifacts

- `summary.json` — pass/fail status, task id, phase-event line count
- `start-task-list.json` — context snapshot at step 2
- `final-task-list.json` — terminal snapshot after `walk-spec`
- `phase-events.jsonl` — typed event stream for the loop
- `steps/*/{command.txt,stdout.json,stderr.log}` — per-step invocation evidence

## Step Mapping

1. `steps/1-shape-spec-title`, `1-shape-spec-task`, `1-shape-spec-demo`
2. `steps/2-resolve-context`
3. `steps/3-do-task-start`, `3-do-task-complete`
4. `steps/4-audit-task`
5. `steps/5-run-demo`
6. `steps/6-audit-spec`
7. `steps/7-walk-spec`
