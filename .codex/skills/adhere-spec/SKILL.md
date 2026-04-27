---
name: adhere-spec
description: Tanren methodology command `adhere-spec`
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

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `adhere-spec`:
The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
- MCP `list_findings` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"spec","check_kind":{"kind":"adherence"}}`
- CLI `list_findings` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"spec","check_kind":{"kind":"adherence"}}'`
- MCP `list_relevant_standards` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}`
- CLI `list_relevant_standards` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" standard list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}'`
- MCP `record_adherence_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}`
- CLI `record_adherence_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" adherence add-finding --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}'`
- MCP `resolve_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}`
- CLI `resolve_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding resolve --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}'`
- MCP `record_finding_still_open` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}`
- CLI `record_finding_still_open` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding still-open --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}'`
- MCP `defer_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}`
- CLI `defer_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding defer --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}'`
- MCP `supersede_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}`
- CLI `supersede_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding supersede --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{"kind":"adherence"}}}'`
- MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
- CLI `list_tasks` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase adhere-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

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
