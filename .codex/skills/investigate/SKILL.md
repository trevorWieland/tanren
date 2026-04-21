---
name: investigate
description: Tanren methodology command `investigate`
role: triage
orchestration_loop: true
autonomy: autonomous
declared_variables:
- READONLY_ARTIFACT_BANNER
- SPEC_VERIFICATION_HOOK
- TASK_TOOL_BINDING
- TASK_VERIFICATION_HOOK
declared_tools:
- revise_task
- create_task
- abandon_task
- add_finding
- escalate_to_blocker
- list_tasks
- report_phase_outcome
required_capabilities:
- task.create
- task.revise
- task.abandon
- finding.add
- phase.escalate
- task.read
- phase.outcome
produces_evidence:
- investigation-report.json (generated from tool calls + narrative)
---

# investigate

## Purpose

Diagnose a specific phase failure. Emit a typed decision: revise the
task, create a new task, abandon with replacements, or — as a last
resort — escalate to a blocker for `resolve-blockers`.

## Inputs (from your dispatch)

- The failing phase (e.g. `task-gate`, `audit-task`, `run-demo`,
  `audit-spec`, `adhere-task`, `adhere-spec`).
- The failing `task_id` (if task-scoped) or spec scope.
- The failure artifacts (gate log, audit findings, demo results,
  adherence findings).
- The diff under suspicion.
- Prior investigation records for this failure signature (the
  orchestrator uses a root-cause fingerprint to enforce a loop cap,
  default 3).

## Responsibilities

1. Read the failure evidence in full. Distinguish root causes from
   symptoms. Do not modify code; this phase is read-only.
2. Classify root causes explicitly. Include BDD-specific classes when
   applicable:
   - missing scenario for claimed behavior
   - weak scenario (mutation survivor)
   - behavior-map drift (implementation and mapping out of sync)
   - acceptance criteria ambiguity
   - environment drift
3. For each root cause, choose one action:
   - **Task scope was wrong:** `revise_task(task_id,
     revised_description, revised_acceptance, reason)`.
   - **A new fix scope is required:** `create_task(title,
     description, origin: Investigation { source_phase,
     source_task, loop_index }, acceptance_criteria)`.
   - **Task is infeasible:** `abandon_task(task_id, reason,
     replacements)` with at least one replacement.
   - **Cannot resolve autonomously:** `escalate_to_blocker(reason,
     options)`.
4. Add `note` / `question` findings for observations that are not
   immediately actionable but might be useful to the next phase.
5. Write a narrative for `investigation-report.json` (tool-generated
   from your calls; the narrative field is your prose).
6. Call `report_phase_outcome("complete", <one-line summary>)`.

## Verification

If you need to reproduce the failure to ground your diagnosis, use
the relevant hook: `just check` or
`just ci` as appropriate. Never modify code.

## Emitting results

mcp

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Implementing fixes (emit a task; `do-task` will execute)
- Editing `plan.md`, `progress.json`, or any orchestrator-owned
  artifact
- Calling any tool outside the `investigate` capability set
  (`complete_task`, `record_rubric_score`, `post_reply_directive`,
  `create_issue` are all denied)
- Chain-escalating repeatedly (if the loop cap is hit, the
  orchestrator promotes to a blocker automatically)
