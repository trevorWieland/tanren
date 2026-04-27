---
name: audit-task
role: audit
orchestration_loop: true
autonomy: autonomous
declared_variables:
- AUDIT_TASK_HOOK
- ISSUE_REF_NOUN
- PILLAR_LIST
- READONLY_ARTIFACT_BANNER
- TASK_TOOL_BINDING
declared_tools:
- add_finding
- list_findings
- resolve_finding
- record_finding_still_open
- defer_finding
- supersede_finding
- record_rubric_score
- list_tasks
- report_phase_outcome
required_capabilities:
- finding.add
- finding.read
- finding.lifecycle
- rubric.record
- task.read
- phase.outcome
produces_evidence:
- audit.md (task-scope narrative body)
---

# audit-task

## Purpose

Apply the opinionated 10-pillar rubric to the task identified in
your dispatch. Emit typed findings per issue. Record a rubric score
per applicable pillar. Do not edit `plan.md`, do not create tasks —
the orchestrator routes failures through `investigate` for autonomous
remediation.

## Inputs (from your dispatch)

- `task_id` and its full record via `list_tasks`.
- `diff_range` — the commit range / file list introduced by this
  task's `do-task` session.
- Relevant standards (for context; standards adherence is a separate
  phase — `adhere-task`).
- `completeness, performance, scalability, strictness, security, stability, maintainability, extensibility, elegance, style, relevance, modularity, documentation_complete` — the effective pillar set (task scope).
- Relevant signposts.
- Projected spec/task artifacts and linked scenarios.

## Responsibilities

1. Read the diff in full. Understand what changed and why.
2. Audit behavior traceability:
   - behavior changes are reflected in projected spec/task artifacts
   - mapped scenarios exist and reflect implemented behavior
   - scenario quality is adequate for claimed behavior
3. Audit mutation quality evidence for touched behavior scope:
   surviving mutants, if any, are explained or addressed.
4. Audit coverage interpretation quality:
   uncovered code is discussed as missing scenario vs dead/non-scenario code.
5. Call `list_findings(status: open, severity: fix_now, scope:
   task, task_id, check_kind: audit)` and recheck each existing
   audit blocker before searching for new findings.
6. For each existing blocker that is fixed, call `resolve_finding`
   with verification evidence. For each blocker still present, call
   `record_finding_still_open` with fresh observation evidence.
   Use `defer_finding` or `supersede_finding` only when the finding
   is intentionally made non-blocking with durable rationale.
7. For each new finding: call `add_finding` with severity
   `fix_now` / `defer` / `note` / `question`, a descriptive title,
   affected files and line numbers, and the pillar it relates to.
   Cross-reference signposts: do not re-surface issues an existing
   signpost records as `deferred` or `architectural_constraint`.
8. For each applicable pillar: call `record_rubric_score(pillar,
   score, rationale, supporting_finding_ids)`.
   - Score 1–10 (target 10, passing 7).
   - `score < target` requires at least one linked finding.
   - `score < passing` requires at least one linked `fix_now`
     finding. Tool will reject invalid linkage.
9. Write narrative reasoning into the body of `audit.md`
   (task-scope section).
10. Call `report_phase_outcome`:
    - `complete` if all scores ≥ passing and zero open blocking audit
    findings remain. The service rejects completion while open
    task-scoped audit `fix_now` findings remain.
    - `blocked` if any `fix_now` findings are produced. The orchestrator
    will dispatch `investigate` to record root cause and repair context,
    then return to `do-task` for this same task.
    - `blocked` if you cannot complete an audit (unusual; document
    in a signpost).

## Verification

If you need to run anything to ground a score, use
`just check`. Do not substitute other commands.

## Emitting results

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `audit-task`:
The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
- MCP `add_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}`
- CLI `add_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding add --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}'`
- MCP `list_findings` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"task","task_id":"00000000-0000-0000-0000-000000000000","check_kind":{"kind":"audit"}}`
- CLI `list_findings` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"task","task_id":"00000000-0000-0000-0000-000000000000","check_kind":{"kind":"audit"}}'`
- MCP `resolve_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}`
- CLI `resolve_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding resolve --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}'`
- MCP `record_finding_still_open` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}`
- CLI `record_finding_still_open` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding still-open --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}'`
- MCP `defer_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}`
- CLI `defer_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding defer --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}'`
- MCP `supersede_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}`
- CLI `supersede_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding supersede --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}'`
- MCP `record_rubric_score` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","pillar":"security","score":8,"target":10,"passing":7,"rationale":"needs additional hardening","supporting_finding_ids":["00000000-0000-0000-0000-000000000000"]}`
- CLI `record_rubric_score` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" rubric record --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","pillar":"security","score":8,"target":10,"passing":7,"rationale":"needs additional hardening","supporting_finding_ids":["00000000-0000-0000-0000-000000000000"]}'`
- MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
- CLI `list_tasks` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","task_id":"00000000-0000-0000-0000-000000000001","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-task --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","task_id":"00000000-0000-0000-0000-000000000001","outcome":{"outcome":"complete","summary":"phase complete"}}'`

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
signposts.md, progress.json, and .tanren-projection-checkpoint.json
are generated from the typed event stream.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Editing `plan.md`, creating tasks, reopening tasks
- Creating `GitHub issues`
- Standards adherence (that's `adhere-task`)
- Committing, pushing, or PR mechanics
- Choosing the next phase
