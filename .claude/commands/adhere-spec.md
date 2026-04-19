---
name: adhere-spec
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
   - `blocked` if any `fix_now` — orchestrator materializes fix tasks.

## Verification

If needed, `just ci`.

## Emitting results

mcp

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Rubric scoring (that's `audit-spec`)
- Authoring new standards
- Editing `plan.md` / creating tasks
- Choosing the next phase
