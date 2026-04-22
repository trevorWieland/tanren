---
agent: feedback
description: Tanren methodology command `handle-feedback`
model: default
subtask: false
template: |2

  # handle-feedback

  ## Purpose

  Triage post-`pull request` review feedback. For each item, classify
  and emit the appropriate typed directive. Tanren-code performs all
  posting, issue creation, and task materialization.

  ## Inputs (from your dispatch)

  - The resolved review context: threads, comments, CI-check failures,
    the spec folder, the diff under review.

  ## Responsibilities

  Classify each review item into exactly one bucket:

  - `valid-actionable` — reviewer is right, code needs change.
    → `create_task(origin: Feedback { source_pr_comment_ref: …})`.
  - `valid-addressed` — reviewer is right but the concern is already
    handled (in code, in signposts, or by design).
    → `post_reply_directive(thread_ref, body, disposition:
    addressed)` with concise references.
  - `invalid` — reviewer is wrong.
    → `post_reply_directive(thread_ref, body, disposition: rebut)`
    with evidence. Be respectful.
  - `style-preference` — subjective, not a correctness concern.
    → `post_reply_directive(thread_ref, body, disposition:
    acknowledged)`.
  - `out-of-scope` — real concern but belongs in a future spec.
    → `create_issue(title, description, suggested_spec_scope,
    priority)` plus `post_reply_directive(thread_ref, body,
    disposition: deferred_to_issue, issue_ref: …)`.
  - `duplicate` — already triaged in this session.
    → no action; log in session summary.

  For CI-check failures: default to `valid-actionable` unless the
  failure is already tracked or is environmental (document in
  session summary and add a signpost via `add_signpost` in a later
  `do-task` session).

  When done: `report_phase_outcome("complete", <session summary>)`.

  ## Emitting results

  Use Tanren MCP tools for all structured mutations (for example `create_task`, `add_finding`, `report_phase_outcome`). CLI fallback uses the same contract:
  `tanren-cli methodology --phase <phase> --spec-id <spec_uuid> --spec-folder <spec_dir> <noun> <verb> --params-file <payload.json>`.

  ⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
  spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
  signposts.md, progress.json, and .tanren-projection-checkpoint.json
  are generated from the typed event stream.
  Postflight reverts unauthorized edits and emits an
  UnauthorizedArtifactEdit event. Use the typed tool surface
  (MCP or CLI) to record progress.


  ## Out of scope

  - Directly posting replies via `gh api` / `linear` / any provider
    shell command
  - Creating `GitHub issues` via shell
  - Editing `plan.md` or other orchestrator-owned artifacts
  - Committing, pushing, merging
  - Deciding workflow progression
---
