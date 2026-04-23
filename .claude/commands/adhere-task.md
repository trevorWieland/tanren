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
   - `blocked` if any `fix_now` findings. Orchestrator will dispatch
     `investigate` for autonomous remediation before resuming the task
     loop.

## Verification

If you need to run a static check to ground a finding, use
`just check`.

## Emitting results

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `adhere-task`:
- MCP `list_relevant_standards` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}`
- CLI `list_relevant_standards` fallback: `tanren-cli methodology --phase adhere-task --spec-id <spec_uuid> --spec-folder <spec_dir> standard list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}'`
- MCP `record_adherence_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}`
- CLI `record_adherence_finding` fallback: `tanren-cli methodology --phase adhere-task --spec-id <spec_uuid> --spec-folder <spec_dir> adherence add-finding --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}'`
- MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
- CLI `list_tasks` fallback: `tanren-cli methodology --phase adhere-task --spec-id <spec_uuid> --spec-folder <spec_dir> task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` fallback: `tanren-cli methodology --phase adhere-task --spec-id <spec_uuid> --spec-folder <spec_dir> phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
signposts.md, progress.json, and .tanren-projection-checkpoint.json
are generated from the typed event stream.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Rubric scoring (that's `audit-task`)
- Authoring new standards (that's `discover-standards` / project)
- Editing `plan.md` or creating tasks
- Choosing the next phase
