---
agent: audit
description: Tanren methodology command `triage-audits`
model: default
subtask: false
template: |2

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

  mcp

  ⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
  plan.md and progress.json are generated from the typed task store.
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
---
