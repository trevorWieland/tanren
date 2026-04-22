---
agent: adherence
description: Tanren methodology command `adhere-spec`
model: default
subtask: false
template: |2

  # adhere-spec

  ## Purpose

  Spec-scope standards compliance check. Same mechanics as
  `adhere-task` but applied to the spec's full accumulated diff.

  ## Inputs (from your dispatch)

  - The spec folder and full spec-scope diff.
  - `list_relevant_standards(spec_id)` → filtered standards.

  ## Responsibilities

  1. For each relevant standard × each file in the spec-scope diff,
     evaluate compliance.
  2. Emit `record_adherence_finding` per violation. Severity rules
     (critical can't defer) match `adhere-task`.
  3. Call `report_phase_outcome`:
     - `complete` if zero `fix_now` findings — spec-level `Adherent`
       guard satisfied.
     - `blocked` if any `fix_now` — orchestrator materializes fix tasks.

  ## Verification

  If needed, `just ci`.

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

  - Rubric scoring (that's `audit-spec`)
  - Authoring new standards
  - Editing `plan.md` / creating tasks
  - Choosing the next phase
---
