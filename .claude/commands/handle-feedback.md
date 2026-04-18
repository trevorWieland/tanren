---
name: handle-feedback
role: feedback
orchestration_loop: true
autonomy: autonomous
declared_variables:
- ISSUE_REF_NOUN
- PR_NOUN
- READONLY_ARTIFACT_BANNER
- TASK_TOOL_BINDING
declared_tools:
- create_task
- create_issue
- post_reply_directive
- list_tasks
- report_phase_outcome
required_capabilities:
- task.create
- issue.create
- feedback.reply
- task.read
- phase.outcome
produces_evidence: []
---

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

- `valid-actionable` â reviewer is right, code needs change.
  â `create_task(origin: Feedback { source_pr_comment_ref: â¦})`.
- `valid-addressed` â reviewer is right but the concern is already
  handled (in code, in signposts, or by design).
  â `post_reply_directive(thread_ref, body, disposition:
  addressed)` with concise references.
- `invalid` â reviewer is wrong.
  â `post_reply_directive(thread_ref, body, disposition: rebut)`
  with evidence. Be respectful.
- `style-preference` â subjective, not a correctness concern.
  â `post_reply_directive(thread_ref, body, disposition:
  acknowledged)`.
- `out-of-scope` â real concern but belongs in a future spec.
  â `create_issue(title, description, suggested_spec_scope,
  priority)` plus `post_reply_directive(thread_ref, body,
  disposition: deferred_to_issue, issue_ref: â¦)`.
- `duplicate` â already triaged in this session.
  â no action; log in session summary.

For CI-check failures: default to `valid-actionable` unless the
failure is already tracked or is environmental (document in
session summary and add a signpost via `add_signpost` in a later
`do-task` session).

When done: `report_phase_outcome("complete", <session summary>)`.

## Emitting results

mcp

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
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
