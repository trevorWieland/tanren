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
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


mcp
