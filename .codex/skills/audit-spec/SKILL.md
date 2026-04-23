---
name: audit-spec
description: Tanren methodology command `audit-spec`
role: audit
orchestration_loop: true
autonomy: autonomous
declared_variables:
- ISSUE_REF_NOUN
- PILLAR_LIST
- READONLY_ARTIFACT_BANNER
- SPEC_VERIFICATION_HOOK
- TASK_TOOL_BINDING
declared_tools:
- add_finding
- record_rubric_score
- record_non_negotiable_compliance
- list_tasks
- report_phase_outcome
required_capabilities:
- finding.add
- rubric.record
- compliance.record
- task.read
- phase.outcome
produces_evidence:
- audit.md (spec-scope narrative body)
---

# audit-spec

## Purpose

Apply the 10-pillar rubric at spec scope. Record non-negotiable
compliance. Classify findings as `fix_now` (must address in this
spec) or `defer` (backlog for future specs via `triage-audits`).

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
5. For each finding: `add_finding` with severity, title,
   affected files/lines, source phase `audit-spec`, the pillar it
   relates to, and `attached_task` if it scopes to one. Cross-
   reference signposts to avoid duplicating known-deferred issues.
6. For each applicable pillar: `record_rubric_score(pillar, score,
   rationale, supporting_finding_ids)`. Same invariants as
   `audit-task` (target 10, passing 7, findings required for gaps,
   `fix_now` required below passing).
7. For each non-negotiable: `record_non_negotiable_compliance(name,
   status, rationale)`.
8. Write reasoning into the `audit.md` body.
9. Call `report_phase_outcome`:
   - `complete` if every pillar ≥ passing, every non-negotiable
     `pass`, demo passed, zero unaddressed `fix_now`.
   - `blocked` otherwise. Orchestrator dispatches `investigate` for
     autonomous remediation and then resumes the loop.

## Verification

Use `just ci` if you need to ground a score by
running the spec-level gate.

## Emitting results

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `audit-spec`:
- MCP `add_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}`
- CLI `add_finding` fallback: `tanren-cli methodology --phase audit-spec --spec-id <spec_uuid> --spec-folder <spec_dir> finding add --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}'`
- MCP `record_rubric_score` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","pillar":"security","score":8,"target":10,"passing":7,"rationale":"needs additional hardening","supporting_finding_ids":["00000000-0000-0000-0000-000000000000"]}`
- CLI `record_rubric_score` fallback: `tanren-cli methodology --phase audit-spec --spec-id <spec_uuid> --spec-folder <spec_dir> rubric record --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","pillar":"security","score":8,"target":10,"passing":7,"rationale":"needs additional hardening","supporting_finding_ids":["00000000-0000-0000-0000-000000000000"]}'`
- MCP `record_non_negotiable_compliance` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","name":"fail-closed-mcp","status":"pass","rationale":"envelope verification is enforced"}`
- CLI `record_non_negotiable_compliance` fallback: `tanren-cli methodology --phase audit-spec --spec-id <spec_uuid> --spec-folder <spec_dir> compliance record --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","name":"fail-closed-mcp","status":"pass","rationale":"envelope verification is enforced"}'`
- MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
- CLI `list_tasks` fallback: `tanren-cli methodology --phase audit-spec --spec-id <spec_uuid> --spec-folder <spec_dir> task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` fallback: `tanren-cli methodology --phase audit-spec --spec-id <spec_uuid> --spec-folder <spec_dir> phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

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
- Editing `roadmap.md`, `plan.md`, or any orchestrator-owned file
- Creating tasks directly
- Standards compliance (that's `adhere-spec`)
- Committing, pushing, or PR mechanics
