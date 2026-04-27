---
agent: meta
description: Tanren methodology command `sync-roadmap`
model: default
subtask: false
template: |2

  # sync-roadmap

  ## Purpose

  Reconcile `tanren/product/roadmap.md` with the real spec-completion
  state held in the Tanren store plus the `GitHub` issue
  source. Emit a structured diff of reconciling actions; Tanren-code
  performs all mutations.

  ## Inputs (from your dispatch)

  - The supplied reconciliation context: current roadmap snapshot,
    issue-source snapshot (filtered to spec-type GitHub issues),
    and the store's spec completion list.
  - Divergences already pre-computed by Tanren-code.

  ## Responsibilities

  1. Read the reconciliation context. Identify:
     - Specs in roadmap but not in the issue source (→ create issue).
     - Issues tagged as specs but missing from roadmap (→ add to
       roadmap).
     - Specs with mismatched status (closed issue but status:planned,
       etc.).
     - Dependency divergences (frontmatter `depends_on` vs issue
       `blockedBy`).
  2. For each reconciling action needed, emit `add_finding` with
     severity `fix_now` or `defer`, tagged with the action shape
     (create/update/relink). Orchestrator applies the mutations.
  3. If user confirmation is needed for a destructive reconciliation
     (e.g. closing a stale roadmap entry), emit
     `post_reply_directive` flagged for the operator.
  4. `report_phase_outcome("complete", <summary>)`.

  ## Emitting results

  Use Tanren MCP tools for all structured mutations in this phase.
  MCP-first canonical invocation set for phase `sync-roadmap`:
  The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
  - MCP `add_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}`
  - CLI `add_finding` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase sync-roadmap --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" finding add --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}'`
  - MCP `post_reply_directive` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","thread_ref":"github:org/repo#123","body":"Thanks for the feedback.","disposition":"ack"}`
  - CLI `post_reply_directive` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase sync-roadmap --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase reply --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","thread_ref":"github:org/repo#123","body":"Thanks for the feedback.","disposition":"ack"}'`
  - MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
  - CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase sync-roadmap --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

  ⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
  spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
  signposts.md, progress.json, and .tanren-projection-checkpoint.json
  are generated from the typed event stream.
  Postflight reverts unauthorized edits and emits an
  UnauthorizedArtifactEdit event. Use the typed tool surface
  (MCP or CLI) to record progress.


  ## Out of scope

  - Calling `GitHub` shell commands directly
  - Editing `roadmap.md` directly (orchestrator does, based on your
    findings)
  - Creating tasks (this command is cross-spec; it creates
    reconciliation findings, not spec-scope tasks)
  - Mutating dependency graphs directly
---
