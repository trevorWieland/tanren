---
name: walk-spec
description: Tanren methodology command `walk-spec`
role: conversation
orchestration_loop: true
autonomy: interactive
declared_variables:
- ISSUE_PROVIDER
- PR_NOUN
- READONLY_ARTIFACT_BANNER
- SPEC_VERIFICATION_HOOK
- TASK_TOOL_BINDING
declared_tools:
- create_task
- list_tasks
- report_phase_outcome
required_capabilities:
- task.create
- task.read
- phase.outcome
produces_evidence:
- none (reads projected artifacts and emits typed outcome/tasks only)
---

# walk-spec

## Purpose

The user's acceptance checkpoint. Walk through behavior outcomes live,
confirm acceptance criteria are met, surface any last concerns, and
signal completion. Tanren-code handles `pull request` creation,
roadmap updates, and `GitHub` communication after you
signal complete.

## Inputs (from your dispatch)

- The fully-implemented spec according to typed `spec status`: all
  tasks Complete, latest spec checks passed, and no open blocking
  findings.
- The spec's projected artifacts: `spec.md`, `plan.md`, `tasks.md`,
  `tasks.json`, `demo.md`, `progress.json`, and `audit.md`.

## Responsibilities

1. Confirm prerequisites from typed `spec status`, not `audit.md`
   frontmatter. All tasks must be `Complete`, latest spec checks must
   be current, and no open blocking findings may remain. If not, call
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

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `walk-spec`:
The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
- MCP `create_task` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"task title","description":"task description","origin":{"kind":"user"},"acceptance_criteria":[]}`
- CLI `create_task` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase walk-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task create --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"task title","description":"task description","origin":{"kind":"user"},"acceptance_criteria":[]}'`
- MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
- CLI `list_tasks` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase walk-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase walk-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
signposts.md, progress.json, and .tanren-projection-checkpoint.json
are generated from the typed event stream.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Creating `pull requests`
- Updating `docs/roadmap/roadmap.md`, issue comments, or any external state
- Running `audit-spec` or any other automated check
- Implementing code (if something breaks, emit a task; do not fix)
