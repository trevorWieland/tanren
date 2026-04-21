---
name: run-demo
description: Tanren methodology command `run-demo`
role: implementation
orchestration_loop: true
autonomy: autonomous
declared_variables:
- ISSUE_REF_NOUN
- READONLY_ARTIFACT_BANNER
- RUN_DEMO_HOOK
- TASK_TOOL_BINDING
declared_tools:
- append_demo_result
- add_finding
- add_signpost
- list_tasks
- report_phase_outcome
required_capabilities:
- demo.results
- finding.add
- signpost.add
- task.read
- phase.outcome
produces_evidence:
- demo.md (narrative body)
- behavior-map.md
---

# run-demo

## Purpose

Execute the demo steps authored in `shape-spec` as a user-visible
behavior walkthrough. Demo execution is not a test-runner proxy.
Record typed results per step and emit findings for failed observables.

## Inputs (from your dispatch)

- The spec folder and its `demo.md` frontmatter (steps with
  `RUN` / `SKIP` modes, descriptions, expected observables).
- The supplied demo environment (already probed by shape-spec).
- `behavior-map.md` for step-to-behavior references.

## Responsibilities

1. Execute every `RUN` step. `SKIP` steps are not executed; they do
   not contribute to pass/fail.
2. Each executed step must reference behavior IDs and expected
   observables from the behavior map.
3. For each executed step: call `append_demo_result(step_id,
   status, observed)` with `pass` or `fail` and the observed
   outcome.
4. For each failure: call `add_finding` with `source_phase:
   run-demo`, descriptive title, affected files (if applicable), and
   severity `fix_now`. If the failure reveals a scenario gap (tests
   pass but demo fails), add a `fix_now` finding describing the
   missing or weak scenario.
5. Add signposts for non-obvious root causes.
6. Call `report_phase_outcome`:
   - `complete` if every `RUN` step passes and at least one `RUN`
     step exists.
   - `blocked` if any `RUN` step fails. Orchestrator will dispatch
     `investigate-spec`.

## Verification

Demo steps run the commands specified in `demo.md`. If you need an
additional gate check, `just ci` is available.

## Emitting results

mcp

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Reclassifying demo steps (the `RUN` / `SKIP` decision was binding
  at `shape-spec` time)
- Editing `plan.md` or creating tasks directly (findings →
  orchestrator → tasks)
- Creating `GitHub issues`
- Committing or pushing
- Choosing the next phase
