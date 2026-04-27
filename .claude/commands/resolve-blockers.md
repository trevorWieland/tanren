---
name: resolve-blockers
role: conversation
orchestration_loop: true
autonomy: interactive
declared_variables:
- ISSUE_REF_NOUN
- READONLY_ARTIFACT_BANNER
- TASK_TOOL_BINDING
declared_tools:
- create_task
- revise_task
- abandon_task
- list_tasks
- report_phase_outcome
required_capabilities:
- task.create
- task.revise
- task.abandon
- task.read
- phase.outcome
produces_evidence: []
---

# resolve-blockers

## Purpose

Interactive escalation-resolution phase. Triggered only after
`investigate` has hit its loop cap and called `escalate_to_blocker`.
Present the investigation-derived options to the user, capture the
chosen path via typed tool calls, then exit so the orchestrator
resumes.

## Inputs (from your dispatch)

- The blocker reason and option list produced by the upstream
  `investigate` call.
- The spec folder state at the time of escalation.
- All prior investigation reports for this fingerprint.

## Responsibilities

1. Summarize the blocker to the user in one paragraph. Pull context
   from the investigation report.
2. Present the options (at least: fix-in-place via new/revised
   task; abandon + replace; defer to future spec). Recommend one.
3. Wait for the user's decision.
4. Apply the chosen path with typed tools:
   - **Fix in place:** `create_task(origin: User)` or
     `revise_task(…)`.
   - **Abandon:** `abandon_task(task_id, reason, replacements)`.
     Replacement tasks must be created first via `create_task`.
   - **Defer to future spec:** `abandon_task` with an
     acknowledgment that no replacement will be created here; the
     user can spin a new spec later.
5. Call `report_phase_outcome("complete", <user-chosen path>)`.

## Out of scope

- Escalating further (resolve-blockers never chain-escalates; if
  the user cannot decide, `report_phase_outcome("error", …)`)
- Editing `plan.md`, `progress.json`, or orchestrator-owned files
- Creating `GitHub issues` directly
- Implementing any code change

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
signposts.md, progress.json, and .tanren-projection-checkpoint.json
are generated from the typed event stream.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `resolve-blockers`:
The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
- MCP `create_task` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"task title","description":"task description","origin":{"kind":"user"},"acceptance_criteria":[]}`
- CLI `create_task` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase resolve-blockers --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task create --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"task title","description":"task description","origin":{"kind":"user"},"acceptance_criteria":[]}'`
- MCP `revise_task` payload: `{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","revised_description":"updated details","revised_acceptance":[],"reason":"clarify acceptance"}`
- CLI `revise_task` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase resolve-blockers --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task revise --json '{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","revised_description":"updated details","revised_acceptance":[],"reason":"clarify acceptance"}'`
- MCP `abandon_task` payload: `{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","reason":"superseded","disposition":"replacement","replacements":[]}`
- CLI `abandon_task` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase resolve-blockers --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task abandon --json '{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","reason":"superseded","disposition":"replacement","replacements":[]}'`
- MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
- CLI `list_tasks` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase resolve-blockers --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase resolve-blockers --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`
