---
name: adhere-spec
role: adherence
orchestration_loop: true
autonomy: autonomous
declared_variables:
  - READONLY_ARTIFACT_BANNER
  - TASK_TOOL_BINDING
declared_tools:
  - list_findings
  - list_relevant_standards
  - record_adherence_finding
  - resolve_finding
  - record_finding_still_open
  - defer_finding
  - supersede_finding
  - list_tasks
  - report_phase_outcome
required_capabilities:
  - finding.read
  - standard.read
  - adherence.record
  - finding.lifecycle
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

1. Call `list_findings(status: open, severity: fix_now, scope:
   spec, check_kind: adherence)` and recheck existing standards
   blockers.
2. Resolve fixed prior blockers with `resolve_finding`; record
   persistent ones with `record_finding_still_open`; defer or
   supersede only with durable evidence.
3. For each relevant standard × each file in the spec-scope diff,
   evaluate compliance.
4. Emit `record_adherence_finding` per new violation. Severity rules
   (critical can't defer) match `adhere-task`.
5. Call `report_phase_outcome`:
   - `complete` if zero open blocking adherence findings — spec-level `Adherent`
     guard satisfied.
   - `blocked` if any `fix_now` — orchestrator dispatches
     `investigate` for autonomous remediation before continuing.

## Verification

Use existing spec-gate evidence from the projected artifacts and event
history. Do not rerun the repository gate from this command; the
orchestrator owns the spec gate.

## Emitting results

{{TASK_TOOL_BINDING}}

{{READONLY_ARTIFACT_BANNER}}

## Out of scope

- Rubric scoring (that's `audit-spec`)
- Authoring new standards
- Editing `plan.md` / creating tasks
- Choosing the next phase
