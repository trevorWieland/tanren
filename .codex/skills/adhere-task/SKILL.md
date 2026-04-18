---
name: adhere-task
role: adherence
orchestration_loop: true
autonomy: autonomous
declared_variables:
- ADHERE_TASK_HOOK
- READONLY_ARTIFACT_BANNER
- TASK_TOOL_BINDING
declared_tools:
- list_relevant_standards
- record_adherence_finding
- list_tasks
- report_phase_outcome
required_capabilities:
- standard.read
- adherence.record
- task.read
- phase.outcome
produces_evidence: []
---

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
2. For each standard + each changed file, evaluate compliance.
3. For each misalignment: call `record_adherence_finding(standard_id,
   affected_files, line_numbers, severity, rationale)`.
   - `fix_now` — violation must be addressed.
   - `defer` — violation is real but acceptable to defer
     (non-critical standards only). Standards with
     `importance: critical` cannot be deferred; the tool enforces
     this.
4. Call `report_phase_outcome`:
   - `complete` if zero `fix_now` adherence findings. The
     `TaskAdherent` guard will be recorded.
   - `fail` if any `fix_now` findings. Orchestrator will materialize
     fix tasks with `origin: Adherence`.

## Verification

If you need to run a static check to ground a finding, use
`just check`.

## Emitting results

mcp

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Rubric scoring (that's `audit-task`)
- Authoring new standards (that's `discover-standards` / project)
- Editing `plan.md` or creating tasks
- Choosing the next phase
