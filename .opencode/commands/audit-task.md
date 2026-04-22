---
agent: audit
description: Tanren methodology command `audit-task`
model: default
subtask: false
template: |2

  # audit-task

  ## Purpose

  Apply the opinionated 10-pillar rubric to the task identified in
  your dispatch. Emit typed findings per issue. Record a rubric score
  per applicable pillar. Do not edit `plan.md`, do not create tasks —
  the orchestrator materializes new tasks from your `fix_now` findings.

  ## Inputs (from your dispatch)

  - `task_id` and its full record via `list_tasks`.
  - `diff_range` — the commit range / file list introduced by this
    task's `do-task` session.
  - Relevant standards (for context; standards adherence is a separate
    phase — `adhere-task`).
  - `completeness, performance, scalability, strictness, security, stability, maintainability, extensibility, elegance, style, relevance, modularity, documentation_complete` — the effective pillar set (task scope).
  - Relevant signposts.
  - Projected spec/task artifacts and linked scenarios.

  ## Responsibilities

  1. Read the diff in full. Understand what changed and why.
  2. Audit behavior traceability:
     - behavior changes are reflected in projected spec/task artifacts
     - mapped scenarios exist and reflect implemented behavior
     - scenario quality is adequate for claimed behavior
  3. Audit mutation quality evidence for touched behavior scope:
     surviving mutants, if any, are explained or addressed.
  4. Audit coverage interpretation quality:
     uncovered code is discussed as missing scenario vs dead/non-scenario code.
  5. For each finding: call `add_finding` with severity
     `fix_now` / `defer` / `note` / `question`, a descriptive title,
     affected files and line numbers, and the pillar it relates to.
     Cross-reference signposts: do not re-surface issues an existing
     signpost records as `deferred` or `architectural_constraint`.
  6. For each applicable pillar: call `record_rubric_score(pillar,
     score, rationale, supporting_finding_ids)`.
     - Score 1–10 (target 10, passing 7).
     - `score < target` requires at least one linked finding.
     - `score < passing` requires at least one linked `fix_now`
       finding. Tool will reject invalid linkage.
  7. Write narrative reasoning into the body of `audit.md`
     (task-scope section).
  8. Call `report_phase_outcome`:
     - `complete` if all scores ≥ passing and zero `fix_now` findings
       remain. The `TaskAudited` guard will be recorded.
     - `blocked` if any `fix_now` findings are produced. The orchestrator
       will materialize fix tasks.
     - `blocked` if you cannot complete an audit (unusual; document
       in a signpost).

  ## Verification

  If you need to run anything to ground a score, use
  `just check`. Do not substitute other commands.

  ## Emitting results

  Use Tanren MCP tools for all structured mutations (for example `create_task`, `add_finding`, `report_phase_outcome`). CLI fallback uses the same contract:
  `tanren methodology --phase <phase> --spec-id <spec_uuid> --spec-folder <spec_dir> <noun> <verb> --json '<payload>'`.

  ⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
  spec.md, plan.md, tasks.md, tasks.json, demo.md, and progress.json
  are generated from the typed event stream.
  Postflight reverts unauthorized edits and emits an
  UnauthorizedArtifactEdit event. Use the typed tool surface
  (MCP or CLI) to record progress.


  ## Out of scope

  - Editing `plan.md`, creating tasks, reopening tasks
  - Creating `GitHub issues`
  - Standards adherence (that's `adhere-task`)
  - Committing, pushing, or PR mechanics
  - Choosing the next phase
---
