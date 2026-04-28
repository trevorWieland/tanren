---
agent: adherence
description: Tanren methodology command `adhere-task`
model: default
subtask: false
template: |2

  # adhere-task

  ## Purpose

  Check the task's diff against the repo's installed standards. Emit
  typed adherence findings (pass/fail per rule). No rubric scores.
  This phase is mechanical compliance, not opinionated judgment.

  ## Inputs (from your dispatch)

  - `task_id` and its diff range.
  - The relevant-standards set via `list_relevant_standards(spec_id)`.

  ## Responsibilities

  1. Fetch relevant standards. The filter already accounts for file
     globs, language, and domain tags — do not reduce further.
  2. Call `list_findings(status: open, severity: fix_now, scope:
     task, task_id, check_kind: adherence)` and recheck each existing
     standards blocker.
  3. Resolve fixed prior blockers with `resolve_finding`; record
     persistent ones with `record_finding_still_open`; defer or
     supersede only with durable evidence.
  4. For each standard + each changed file, evaluate compliance.
  5. For each new misalignment: call `record_adherence_finding(standard_id,
     affected_files, line_numbers, severity, rationale, attached_task)`.
     - `fix_now` — violation must be addressed.
     - `defer` — violation is real but acceptable to defer
       (non-critical standards only). Standards with
       `importance: critical` cannot be deferred; the tool enforces
       this.
  6. Call `report_phase_outcome`:
     - `complete` if zero open blocking adherence findings remain. The
       service rejects completion while open task-scoped adherence
       `fix_now` findings remain.
     - `blocked` if any `fix_now` findings remain. Orchestrator will dispatch
       `investigate` to record root cause and repair context, then return to
       `do-task` for this same task.

  ## Verification

  If you need to run a static check to ground a finding, use
  `just check`.

  ## Emitting results

  Use Tanren MCP tools for all structured mutations in this phase.
  MCP-first canonical invocation set for phase `adhere-task`:
  The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
  - MCP `list_findings` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"task","task_id":"00000000-0000-0000-0000-000000000000","check_kind":{"kind":"adherence"}}`
  - CLI `list_findings` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"task","task_id":"00000000-0000-0000-0000-000000000000","check_kind":{"kind":"adherence"}}'`
  - MCP `list_relevant_standards` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}`
  - CLI `list_relevant_standards` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" standard list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}'`
  - MCP `record_adherence_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}`
  - CLI `record_adherence_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" adherence add-finding --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}'`
  - MCP `resolve_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}`
  - CLI `resolve_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding resolve --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}'`
  - MCP `record_finding_still_open` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}`
  - CLI `record_finding_still_open` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding still-open --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}'`
  - MCP `defer_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}`
  - CLI `defer_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding defer --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}'`
  - MCP `supersede_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}`
  - CLI `supersede_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding supersede --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}'`
  - MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
  - CLI `list_tasks` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
  - MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","task_id":"00000000-0000-0000-0000-000000000001","outcome":{"outcome":"complete","summary":"phase complete"}}`
  - CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","task_id":"00000000-0000-0000-0000-000000000001","outcome":{"outcome":"complete","summary":"phase complete"}}'`

  ⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
  spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
  signposts.md, progress.json, and .tanren-projection-checkpoint.json
  are generated from the typed event stream.
  Postflight reverts unauthorized edits and emits an
  UnauthorizedArtifactEdit event. Use the typed tool surface
  (MCP or CLI) to record progress.


  ## Out of scope

  - Rubric scoring (that's `audit-task`)
  - Authoring new standards (that belongs to a future project-planning flow)
  - Editing `plan.md` or creating tasks
  - Choosing the next phase
---
