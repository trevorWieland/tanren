---
name: shape-spec
description: Tanren methodology command `shape-spec`
role: conversation
orchestration_loop: true
autonomy: interactive
declared_variables:
- ISSUE_PROVIDER
- ISSUE_REF_NOUN
- READONLY_ARTIFACT_BANNER
- TASK_TOOL_BINDING
declared_tools:
- set_spec_title
- set_spec_non_negotiables
- add_spec_acceptance_criterion
- set_spec_demo_environment
- set_spec_dependencies
- set_spec_base_branch
- add_demo_step
- mark_demo_step_skip
- create_task
- report_phase_outcome
required_capabilities:
- spec.frontmatter
- demo.frontmatter
- task.create
- task.read
- phase.outcome
produces_evidence:
- spec.md (narrative body)
- demo.md (narrative body)
---

# shape-spec

## Purpose

Shape a new spec interactively with the user. Establish scope,
non-negotiables, acceptance criteria, a runnable demo plan, and an
initial task breakdown. You do **not** create issues, branches, or
PRs; Tanren-code owns all workflow mechanics.

## Inputs (from your dispatch)

- A `GitHub` GitHub issue reference (id, title,
  body). Tanren-code has already resolved and supplied this.
- The effective repo profile and standards index.

## Responsibilities

1. Work with the user to articulate the problem, scope, and
   acceptance criteria. Ask clarifying questions until there is zero
   ambiguity.
2. Derive non-negotiables (hard constraints that must always hold).
3. Design a runnable demo plan: concrete steps, each tagged `RUN`
   or `SKIP`, with explicit expected observables. Probe the demo
   environment *before* committing `RUN` tags — if a connection is
   unavailable, mark `SKIP` with the reason.
4. Break the work into ordered tasks with clear acceptance criteria.
   Tasks should be independently verifiable.
5. Emit every structured fact via tools (see below). Author the
   narrative body of `spec.md` and the narrative portions of
   `demo.md` as supporting prose.

## Emitting results

mcp

Call in this order:

1. `set_spec_title`, `set_spec_non_negotiables`
2. `add_spec_acceptance_criterion` per criterion (stable id, clear
   description, verifiable measurable)
3. `set_spec_demo_environment` with probed connection defs
4. `add_demo_step` per `RUN` step; `mark_demo_step_skip` per `SKIP`
   with a reason
5. `set_spec_dependencies` if this spec depends on other specs or
   external GitHub issues
6. `set_spec_base_branch` with the branch this spec will target
7. `create_task` per planned task with stable ordering, typed
   `origin: ShapeSpec`, and explicit acceptance criteria
8. `report_phase_outcome("complete", <short summary>)`

The narrative body of `spec.md` captures motivation, background, and
the scope conversation. The narrative body of `demo.md` captures the
walkthrough story.

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Creating or mutating `GitHub` GitHub issues
- Creating or checking out branches
- Commit, push, or PR operations
- Editing `plan.md`, `progress.json`, or any other orchestrator-owned
  artifact
- Selecting or executing verification hooks
- Choosing the next workflow step
