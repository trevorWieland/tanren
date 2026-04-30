---
agent: audit
description: Tanren methodology command `audit-spec`
model: default
subtask: false
template: |2

  # audit-spec

  ## Purpose

  Apply the 10-pillar rubric at spec scope. Record non-negotiable
  compliance. Classify findings as `fix_now` (must address in this
  spec) or `defer` (backlog for future specs through project intake).

  ## Inputs (from your dispatch)

  - The full spec folder and its accumulated diff.
  - `list_tasks(filter: {spec_id})` for completion state.
  - Relevant standards (for context; compliance is `adhere-spec`).
  - `completeness, performance, scalability, strictness, security, stability, maintainability, extensibility, elegance, style, relevance, modularity, documentation_complete` — effective pillar set for spec scope.
  - The spec's non-negotiables (from spec frontmatter).
  - Projected spec/task artifacts, linked scenarios, and demo outcomes.

  ## Responsibilities

  1. Review the full spec's diff against the spec's acceptance
     criteria, non-negotiables, and pillar expectations.
  2. Verify behavior coverage integrity at spec scope:
     - every shaped behavior is mapped
     - mapped scenarios exist and pass
     - deprecated behaviors are intentionally handled
  3. Verify mutation evidence quality at spec scope:
     surviving mutants are triaged and mapped to concrete follow-up
     actions when needed.
  4. Verify coverage-gap interpretation quality:
     uncovered paths are classified as missing scenario vs dead/non-
     scenario support code.
  5. Call `list_findings(status: open, severity: fix_now, scope:
     spec, check_kind: audit)` and recheck existing audit blockers.
  6. Resolve fixed prior blockers with `resolve_finding`; record
     persistent ones with `record_finding_still_open`; defer or
     supersede only with durable evidence.
  7. For each new finding: `add_finding` with severity, title,
     affected files/lines, source phase `audit-spec`, the pillar it
     relates to, and `attached_task` if it scopes to one. Cross-
     reference signposts to avoid duplicating known-deferred issues.
  8. For each applicable pillar: `record_rubric_score(pillar, score,
     rationale, supporting_finding_ids)`. Same invariants as
     `audit-task` (target 10, passing 7, findings required for gaps,
     `fix_now` required below passing).
  9. For each non-negotiable: `record_non_negotiable_compliance(name,
     status, rationale)`. Use a stable short slug for `name`; put the
     full non-negotiable text and evidence in `rationale`.
  10. Write reasoning into the `audit.md` body.
  11. Call `report_phase_outcome`:
      - `complete` if every pillar ≥ passing, every non-negotiable
      `pass`, demo passed, zero open blocking audit `fix_now`.
      - `blocked` otherwise. Orchestrator dispatches `investigate` for
      autonomous remediation and then resumes the loop.

  ## Verification

  Use existing spec-gate evidence from the projected artifacts and event
  history. Do not rerun the repository gate from this command; the
  orchestrator owns the spec gate.

  ## Emitting results

  Use Tanren MCP tools for all structured mutations in this phase.
  MCP-first canonical invocation set for phase `audit-spec`:
  The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
  - MCP `add_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}`
  - CLI `add_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding add --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}'`
  - MCP `list_findings` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"spec","check_kind":{"kind":"audit"}}`
  - CLI `list_findings` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"spec","check_kind":{"kind":"audit"}}'`
  - MCP `resolve_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}`
  - CLI `resolve_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding resolve --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}'`
  - MCP `record_finding_still_open` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}`
  - CLI `record_finding_still_open` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding still-open --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}'`
  - MCP `defer_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}`
  - CLI `defer_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding defer --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}'`
  - MCP `supersede_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}`
  - CLI `supersede_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding supersede --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{"kind":"audit"}}}'`
  - MCP `record_rubric_score` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","pillar":"security","score":8,"target":10,"passing":7,"rationale":"needs additional hardening","supporting_finding_ids":["00000000-0000-0000-0000-000000000000"]}`
  - CLI `record_rubric_score` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" rubric record --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","pillar":"security","score":8,"target":10,"passing":7,"rationale":"needs additional hardening","supporting_finding_ids":["00000000-0000-0000-0000-000000000000"]}'`
  - MCP `record_non_negotiable_compliance` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","name":"fail-closed-mcp","status":"pass","rationale":"envelope verification is enforced"}`
  - CLI `record_non_negotiable_compliance` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" compliance record --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","name":"fail-closed-mcp","status":"pass","rationale":"envelope verification is enforced"}'`
  - MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
  - CLI `list_tasks` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
  - MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
  - CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase audit-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

  ⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
  spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
  signposts.md, progress.json, and .tanren-projection-checkpoint.json
  are generated from the typed event stream.
  Postflight reverts unauthorized edits and emits an
  UnauthorizedArtifactEdit event. Use the typed tool surface
  (MCP or CLI) to record progress.


  ## Out of scope

  - Creating `GitHub issues` for deferred items (orchestrator
    does this via `create_issue` on your classified findings)
  - Editing `docs/roadmap/roadmap.md`, `plan.md`, or any orchestrator-owned file
  - Creating tasks directly
  - Standards compliance (that's `adhere-spec`)
  - Committing, pushing, or PR mechanics
---
