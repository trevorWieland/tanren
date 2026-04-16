# Lane 0.5 — Methodology Boundary and Self-Hosting — Agent Brief

## Task

Define and then execute the methodology-boundary lane so workflow mechanics are
treated as Tanren-code responsibilities rather than prompt-local behavior.
This planning pass only establishes that scope; lane 0.5 itself will apply the
shared command markdown changes.

## Full Spec

Read `docs/rewrite/tasks/LANE-0.5-METHODOLOGY.md` completely before starting.
Also read:

1. `docs/rewrite/METHODOLOGY_BOUNDARY.md`
2. `docs/rewrite/HLD.md`
3. `docs/rewrite/DESIGN_PRINCIPLES.md`
4. `docs/methodology/system.md`
5. `docs/architecture/phase-taxonomy.md`

## Key Context

- Lane 0.4 remains the Rust dispatch CRUD slice.
- Lane 0.5 owns the methodology boundary and the follow-on shared-command
  refactor.
- Shared command markdown must define agent behavior only.
- Workflow mechanics belong in code even where the current Python system has
  not fully caught up yet.
- A `do-task` agent should be told what task to execute; it should not discover
  the next task for itself.
- Gate selection, issue-tracker behavior, branch/publication workflow, and
  review/reply mechanics belong to Tanren-code, not the prompts.

## Deliverables

| Area | Deliverable |
|------|-------------|
| Rewrite canon | HLD, design principles, roadmap, crate guide, methodology-boundary doc |
| Methodology docs | Updated ownership and verification-hook docs |
| Lane 0.5 execution scope | Explicit command-level refactor plan describing what must move from markdown into code |
| Shared command markdown | Future lane 0.5 edits that remove literal issue/gate/SCM workflow instructions and replace them with workflow-context abstractions |
| Lane docs | New lane 0.5 docs and refined lane 0.4 scope |

## Required Command-Level Outcomes

Lane 0.5 must leave the shared commands in this shape:

1. `shape-spec` defines the spec with the user, but Tanren-code owns issue
   creation/fetch, candidate selection, dependency mutation, branch prep, and
   publication setup.
2. `do-task` receives an explicit task target and resolved verification hook.
   It does not choose the next task, choose its own gate, or commit/push.
3. `audit-task` receives an explicit task/diff target and emits findings.
   Tanren-code owns fix-item routing and workflow mutation.
4. `run-demo` executes the supplied demo context and records findings. It does
   not decide routing or workflow state.
5. `audit-spec` produces whole-spec findings and fix/defer classification, but
   Tanren-code owns deferred-work creation and workflow mutation.
6. `walk-spec` is the human validation checkpoint only. Review/publication
   mechanics are handled by Tanren-code or the human outside the prompt.
7. `handle-feedback`, `sync-roadmap`, and similar workflow-heavy commands must
   become workflow-context consumers rather than shells around GitHub commands.

## Non-Negotiables

1. **Docs only.** Do not implement runtime or installer behavior in this lane.
2. **No ambiguity.** The tanren-code vs tanren-markdown split must be explicit.
3. **No mixed ownership.** If a responsibility depends on provider, repo,
   branch, workflow state, or verification command choice, it belongs to code.
4. **Keep 0.4 narrow.** Do not smuggle methodology work into the 0.4 scope.
5. **No preemptive command edits in planning.** This planning/doc pass defines
   the lane; the actual shared-command edits happen when lane 0.5 is executed.

## Done When

1. Rewrite canon documents the boundary consistently.
2. Methodology docs describe command/phase-keyed verification-hook resolution.
3. The lane 0.5 brief clearly specifies the future shared-command refactor,
   including task-selection, gate-resolution, issue-provider, and
   publication-workflow ownership.
4. Lane 0.4 and lane 0.5 scopes are clearly separated.
5. Lane 0.5 documents manual self-hosting as the pre-Phase-1 target state.
