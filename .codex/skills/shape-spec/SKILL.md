---
name: shape-spec
description: Tanren methodology command `shape-spec`
role: conversation
orchestration_loop: true
autonomy: interactive
declared_variables:
- ISSUE_REF_NOUN
- READONLY_ARTIFACT_BANNER
- TASK_TOOL_BINDING
declared_tools:
- set_spec_title
- set_spec_problem_statement
- set_spec_motivations
- set_spec_expectations
- set_spec_planned_behaviors
- set_spec_implementation_plan
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
- spec.md (generated projection)
- plan.md (generated projection)
- tasks.md (generated projection)
- tasks.json (generated projection)
- demo.md (generated projection)
- progress.json (generated projection)
---

# shape-spec

## Purpose

Shape a new spec interactively with the user. Establish scope,
problem framing, motivations, expectations, planned behaviors,
ordered implementation plan, non-negotiables, acceptance criteria,
a runnable demo plan, and an initial task breakdown.

## Inputs (from your dispatch)

- A `GitHub issue` reference (id, title,
  body). Tanren-code has already resolved and supplied this.
- The effective repo profile and standards index.

## Responsibilities

1. Work with the user to articulate the problem, scope, and
   acceptance criteria. Ask clarifying questions until there is zero
   ambiguity.
2. Before calling any mutation tool, present a draft bundle
   (title, problem statement, motivations, expectations, planned
   behaviors, ordered implementation plan, non-negotiables,
   acceptance criteria, demo plan, ordered tasks) and get explicit
   user confirmation to proceed.
3. Derive non-negotiables (hard constraints that must always hold).
4. Capture planned behaviors as typed list entries tied to the shaped
   scope.
5. Design a runnable demo plan: concrete steps, each tagged `RUN`
   or `SKIP`, with explicit expected observables. Probe the demo
   environment *before* committing `RUN` tags —
   if a connection is unavailable, mark `SKIP` with the reason. Demo
   steps are proof of completed behavior only; do not put
   implementation actions (for example "delete files") in demo steps.
   Use verification observables instead (for example grep/assert
   checks showing the final state).
6. Break the work into ordered tasks with clear acceptance criteria.
   Tasks should be independently verifiable and traceable to planned
   behaviors and expectations.
7. Emit every structured fact via tools (see below). Do not hand-edit
   orchestrator-owned artifacts.

## Emitting results

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `shape-spec`:
The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
- MCP `set_spec_title` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"Spec title"}`
- CLI `set_spec_title` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-title --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"Spec title"}'`
- MCP `set_spec_problem_statement` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","problem_statement":"Problem statement"}`
- CLI `set_spec_problem_statement` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-problem-statement --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","problem_statement":"Problem statement"}'`
- MCP `set_spec_motivations` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","motivations":["motivation"]}`
- CLI `set_spec_motivations` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-motivations --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","motivations":["motivation"]}'`
- MCP `set_spec_expectations` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","expectations":["expectation"]}`
- CLI `set_spec_expectations` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-expectations --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","expectations":["expectation"]}'`
- MCP `set_spec_planned_behaviors` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","planned_behaviors":["behavior"]}`
- CLI `set_spec_planned_behaviors` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-planned-behaviors --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","planned_behaviors":["behavior"]}'`
- MCP `set_spec_implementation_plan` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","implementation_plan":["step 1"]}`
- CLI `set_spec_implementation_plan` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-implementation-plan --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","implementation_plan":["step 1"]}'`
- MCP `set_spec_non_negotiables` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","items":["non-negotiable"]}`
- CLI `set_spec_non_negotiables` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-non-negotiables --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","items":["non-negotiable"]}'`
- MCP `add_spec_acceptance_criterion` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","criterion":{"id":"ac-1","description":"criterion","measurable":"observable evidence"}}`
- CLI `add_spec_acceptance_criterion` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec add-acceptance-criterion --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","criterion":{"id":"ac-1","description":"criterion","measurable":"observable evidence"}}'`
- MCP `set_spec_demo_environment` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","demo_environment":{"connections":[{"name":"api","kind":"http","probe":"GET /healthz"}]}}`
- CLI `set_spec_demo_environment` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-demo-environment --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","demo_environment":{"connections":[{"name":"api","kind":"http","probe":"GET /healthz"}]}}'`
- MCP `set_spec_dependencies` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","dependencies":{"depends_on_spec_ids":[],"external_issue_refs":[]}}`
- CLI `set_spec_dependencies` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-dependencies --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","dependencies":{"depends_on_spec_ids":[],"external_issue_refs":[]}}'`
- MCP `set_spec_base_branch` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","branch":"main"}`
- CLI `set_spec_base_branch` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" spec set-base-branch --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","branch":"main"}'`
- MCP `add_demo_step` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","id":"step-1","mode":"RUN","description":"Run smoke flow","expected_observable":"No errors"}`
- CLI `add_demo_step` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" demo add-step --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","id":"step-1","mode":"RUN","description":"Run smoke flow","expected_observable":"No errors"}'`
- MCP `mark_demo_step_skip` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","step_id":"step-1","reason":"not applicable"}`
- CLI `mark_demo_step_skip` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" demo mark-step-skip --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","step_id":"step-1","reason":"not applicable"}'`
- MCP `create_task` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"task title","description":"task description","origin":{"kind":"user"},"acceptance_criteria":[]}`
- CLI `create_task` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" task create --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"task title","description":"task description","origin":{"kind":"user"},"acceptance_criteria":[]}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase shape-spec --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

Do not emit mutation calls until the user has confirmed the shaped
draft.

Call in this order:

1. `set_spec_title`, `set_spec_problem_statement`,
   `set_spec_motivations`, `set_spec_expectations`,
   `set_spec_planned_behaviors`, `set_spec_implementation_plan`,
   `set_spec_non_negotiables`
2. `add_spec_acceptance_criterion` per criterion (stable id, clear
   description, verifiable measurable)
3. `set_spec_demo_environment` with probed connection defs
4. `add_demo_step` per `RUN` step; `mark_demo_step_skip` per `SKIP`
   with a reason
5. `set_spec_dependencies` if this spec depends on other specs or
   external GitHub issues
6. `set_spec_base_branch` with the branch this spec will target
7. `create_task` per planned task with stable ordering, typed
   `origin: ShapeSpec`, explicit acceptance criteria, and behavior
   coverage intent
8. `report_phase_outcome("complete", <short summary>)`

Successful completion must leave the full generated artifact set
present and current: `spec.md`, `plan.md`, `tasks.md`, `tasks.json`,
`demo.md`, `progress.json`, `phase-events.jsonl`.

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
signposts.md, progress.json, and .tanren-projection-checkpoint.json
are generated from the typed event stream.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Handoff expectation

Shape-spec ends at artifact/task creation plus
`report_phase_outcome("complete", …)`. The expected execution handoff
is:

1. `do-task` on the first pending task (or orchestrator auto-selection
   when available)
2. Task loop execution until all tasks are complete
3. Spec gate + `run-demo` + `audit-spec` after implementation is done

## Out of scope

- Creating or mutating `GitHub issues
- Creating or checking out branches
- Commit, push, or PR operations
- Editing `plan.md`, `progress.json`, or any other orchestrator-owned
  artifact
- Selecting or executing verification hooks
- Choosing the next workflow step
