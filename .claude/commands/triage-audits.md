---
name: triage-audits
role: audit
orchestration_loop: false
autonomy: interactive
declared_variables:
- ISSUE_PROVIDER
- ISSUE_REF_NOUN
- READONLY_ARTIFACT_BANNER
- STANDARDS_ROOT
- TASK_TOOL_BINDING
declared_tools:
- add_finding
- create_issue
- report_phase_outcome
required_capabilities:
- finding.add
- issue.create
- phase.outcome
produces_evidence: []
---

# triage-audits

## Purpose

Convert a batch standards-audit report (codebase-wide, run on demand
or on a schedule) into prioritized backlog `GitHub issues` for
future specs. This is **backlog curation**, not spec-loop work —
nothing here affects the active spec.

## Inputs (from your dispatch)

- The latest batch audit reports under
  `tanren/standards/audits/{date}/`.
- The currently installed standards index.

## Responsibilities

1. Parse all audit reports. Extract per-standard scores, violation
   counts, file lists.
2. Score each standard's priority: `priority = (target - score) *
   importance_weight`.
3. Group violations by **root cause / natural fix scope**, not
   per-standard. Example: "Modernize type annotations in
   `packages/foo/`" as one group, rather than one issue per standard
   that touches the same files.
4. Present the proposed issue groups to the user, ordered by
   priority. User approves, skips, or adjusts each group.
5. For each approved group: `create_issue(title, description,
   suggested_spec_scope, priority)`. These are backlog items, not
   tasks in the current spec. `shape-spec` will eventually pick them
   up.
6. `add_finding(severity: note)` per cross-cutting observation
   that doesn't warrant its own issue.
7. `report_phase_outcome("complete", <summary>)`.

## Emitting results

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `triage-audits`:
The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
- MCP `add_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}`
- CLI `add_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase triage-audits --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding add --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}'`
- MCP `create_issue` payload: `{"schema_version":"1.0.0","origin_spec_id":"00000000-0000-0000-0000-000000000000","title":"Follow-up","description":"Track deferred work","suggested_spec_scope":"future-spec","priority":"medium"}`
- CLI `create_issue` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase triage-audits --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" issue create --json '{"schema_version":"1.0.0","origin_spec_id":"00000000-0000-0000-0000-000000000000","title":"Follow-up","description":"Track deferred work","suggested_spec_scope":"future-spec","priority":"medium"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase triage-audits --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
signposts.md, progress.json, and .tanren-projection-checkpoint.json
are generated from the typed event stream.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Creating tasks (`create_task` is denied for this command — tasks
  belong to active specs; triage output is backlog issues)
- Editing `roadmap.md`, `plan.md`, or any orchestrator-owned file
- Calling `GitHub` shell commands directly
- Modifying standards (that's `discover-standards` /
  `inject-standards`)
