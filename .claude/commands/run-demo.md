---
name: run-demo
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
- demo.md (projected frontmatter + narrative body)
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
- Projected spec/task artifacts for expectations and task state
  context.

## Responsibilities

1. Execute every `RUN` step. `SKIP` steps are not executed; they do
   not contribute to pass/fail.
2. Each executed step must validate expected observables from shaped
   spec/demos and reflect current projected task/spec context.
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

Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `run-demo`:
- MCP `append_demo_result` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","step_id":"step-1","status":"pass","observed":"all checks green"}`
- CLI `append_demo_result` fallback: `tanren-cli methodology --phase run-demo --spec-id <spec_uuid> --spec-folder <spec_dir> demo append-result --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","step_id":"step-1","status":"pass","observed":"all checks green"}'`
- MCP `add_finding` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}`
- CLI `add_finding` fallback: `tanren-cli methodology --phase run-demo --spec-id <spec_uuid> --spec-folder <spec_dir> finding add --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}'`
- MCP `add_signpost` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"unresolved","problem":"problem statement","evidence":"evidence summary","tried":[],"files_affected":[]}`
- CLI `add_signpost` fallback: `tanren-cli methodology --phase run-demo --spec-id <spec_uuid> --spec-folder <spec_dir> signpost add --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"unresolved","problem":"problem statement","evidence":"evidence summary","tried":[],"files_affected":[]}'`
- MCP `list_tasks` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}`
- CLI `list_tasks` fallback: `tanren-cli methodology --phase run-demo --spec-id <spec_uuid> --spec-folder <spec_dir> task list --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}'`
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` fallback: `tanren-cli methodology --phase run-demo --spec-id <spec_uuid> --spec-folder <spec_dir> phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
signposts.md, progress.json, and .tanren-projection-checkpoint.json
are generated from the typed event stream.
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
