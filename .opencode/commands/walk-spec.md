---
agent: conversation
description: Tanren methodology command `walk-spec`
model: default
subtask: false
template: |2

  # walk-spec

  ## Purpose

  The user's acceptance checkpoint. Walk through behavior outcomes live,
  confirm acceptance criteria are met, surface any last concerns, and
  signal completion. Tanren-code handles `pull request` creation,
  roadmap updates, and `GitHub` communication after you
  signal complete.

  ## Inputs (from your dispatch)

  - The fully-implemented spec (all tasks Complete, audits passed,
    demo passed).
  - The spec's projected artifacts: `spec.md`, `plan.md`, `tasks.md`,
    `tasks.json`, `demo.md`, `progress.json`, and `audit.md`.

  ## Responsibilities

  1. Confirm prerequisites: all tasks `Complete`, `audit-spec` status
     `pass`, demo status `pass`. If not, call
     `report_phase_outcome("error", …)` immediately — walk-spec is
     not the place to fix unfinished work.
  2. Run `just ci` and confirm green.
  3. Present an implementation summary in shaped-behavior terms:
     planned behaviors, implemented tasks, and demo evidence.
  4. Walk through the demo step-by-step. For each step: explain,
     execute, show result, confirm before next.
  5. If a demo step fails during the walkthrough: STOP. Call
     `create_task(title, description, origin: User)` with the
     observed failure, then `report_phase_outcome("blocked", …)`. Do not
     silently fix.
  6. If the user accepts: `report_phase_outcome("complete", …)`.
     Tanren-code will create the `pull request`, update roadmap state,
     and post any required status comments.
  7. If the user rejects: `create_task(origin: User)` with the user's
     concern; `report_phase_outcome("blocked", …)`.

  ## Verification

  `just ci`.

  ## Emitting results

  Use Tanren MCP tools for all structured mutations (for example `create_task`, `add_finding`, `report_phase_outcome`). CLI fallback uses the same contract:
  `tanren-cli methodology --phase <phase> --spec-id <spec_uuid> --spec-folder <spec_dir> <noun> <verb> --params-file <payload.json>`.

  ⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
  spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
  signposts.md, progress.json, and .tanren-projection-checkpoint.json
  are generated from the typed event stream.
  Postflight reverts unauthorized edits and emits an
  UnauthorizedArtifactEdit event. Use the typed tool surface
  (MCP or CLI) to record progress.


  ## Out of scope

  - Creating `pull requests`
  - Updating `roadmap.md`, issue comments, or any external state
  - Running `audit-spec` or any other automated check
  - Implementing code (if something breaks, emit a task; do not fix)
---
