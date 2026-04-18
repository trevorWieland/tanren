---
name: do-task
role: implementation
orchestration_loop: true
autonomy: autonomous
declared_variables:
  - ISSUE_REF_NOUN
  - PR_NOUN
  - READONLY_ARTIFACT_BANNER
  - TASK_TOOL_BINDING
  - TASK_VERIFICATION_HOOK
declared_tools:
  - start_task
  - complete_task
  - add_signpost
  - update_signpost_status
  - list_tasks
  - report_phase_outcome
required_capabilities:
  - task.start
  - task.complete
  - signpost.add
  - signpost.update
  - task.read
  - phase.outcome
produces_evidence:
  - signposts.md (narrative body)
---

# do-task

## Purpose

Implement the single task identified in your dispatch context.
Nothing more. Task selection, gate execution, commits, pushes, and
workflow progression are Tanren-code's job.

## Inputs (from your dispatch)

- The `task_id` to implement, with full typed description and
  acceptance criteria. Use `list_tasks` to fetch the record.
- The spec folder path.
- Relevant standards (injected separately by Tanren-code; treat as
  context, not edits).

## Responsibilities

1. Call `start_task(task_id)` at session start (if not already
   transitioned).
2. Implement only the supplied task. Do not touch unrelated files.
3. Run `{{TASK_VERIFICATION_HOOK}}` before signalling complete. If
   it fails on trivial issues (formatting, imports), self-fix and
   re-run. If it fails persistently, stop: emit a signpost and
   report `blocked` (Tanren-code will dispatch `investigate`).
4. Record signposts for non-obvious issues you hit or decisions that
   would surprise a future reader. Each signpost needs concrete
   evidence — error messages, file paths, command output.
5. On successful implementation: call
   `complete_task(task_id, evidence_refs)` with the relevant file
   paths / commit refs. The `Implemented` transition is recorded by
   Tanren-code; the gate / audit / adherence guards run in parallel
   afterward.
6. Call `report_phase_outcome("complete", …)`.

## Verification

Run `{{TASK_VERIFICATION_HOOK}}` locally. Do not substitute other
commands; Tanren-code has chosen this hook specifically for the
`do-task` phase.

## Emitting results

{{TASK_TOOL_BINDING}}

Signposts carry typed status: `unresolved`, `resolved`, `deferred`,
`architectural_constraint`. Use them honestly — they feed future
audits and investigations.

{{READONLY_ARTIFACT_BANNER}}

## Out of scope

- Choosing the next task (Tanren-code will dispatch another
  `do-task` if more tasks remain)
- Editing `plan.md`, `progress.json`, or any orchestrator-owned
  artifact
- Creating, checking out, committing, pushing, or merging branches
- Opening or modifying `{{ISSUE_REF_NOUN}}s` or `{{PR_NOUN}}s`
- Recording rubric scores or findings (that's `audit-task`)
- Checking standards adherence (that's `adhere-task`)
