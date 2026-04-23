---
name: adhere-spec
description: Tanren methodology command `adhere-spec`
role: adherence
orchestration_loop: true
autonomy: autonomous
declared_variables:
- ADHERE_SPEC_HOOK
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
   - `blocked` if any `fix_now` — orchestrator dispatches
     `investigate` for autonomous remediation before continuing.

## Verification

If needed, `just ci`.

## Emitting results

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `adhere-spec`:
- MCP `list_relevant_standards` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}`
- CLI `list_relevant_standards` fallback: `tanren-cli methodology --phase adhere-spec --spec-id <spec_uuid> --spec-folder <spec_dir> standard list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}'`
- MCP `record_adherence_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}`
- CLI `record_adherence_finding` fallback: `tanren-cli methodology --phase adhere-spec --spec-id <spec_uuid> --spec-folder <spec_dir> adherence add-finding --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}'`
- MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
- CLI `list_tasks` fallback: `tanren-cli methodology --phase adhere-spec --spec-id <spec_uuid> --spec-folder <spec_dir> task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` fallback: `tanren-cli methodology --phase adhere-spec --spec-id <spec_uuid> --spec-folder <spec_dir> phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

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
