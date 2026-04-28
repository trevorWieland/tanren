# Spec Lifecycle

Tanren is intentionally opinionated about this lifecycle. A spec is not an
arbitrary task bundle; it is the executable slice of one roadmap DAG node, and
that node must complete at least one accepted behavior.

## Upstream Context

A spec normally originates from one roadmap DAG node. That node must complete
at least one accepted behavior and declare its expected behavior evidence.
`shape-spec` operationalizes the node into acceptance criteria, demo steps,
tasks, and dependencies; it should not invent product behavior from scratch
unless it is explicitly handling an intake or planning-change flow.

Roadmap nodes can be created from initial product planning, accepted external
intake, or proactive project analysis such as scheduled standards sweeps,
security audits, mutation-testing reports, and post-ship health checks. In all
cases the spec still needs a behavior-backed demo story; work that completes
no behavior is too thin to enter the executable roadmap.

## Core Lifecycle

`draft -> shaped -> executing -> validating -> review -> merged`

The lifecycle exists to preserve evidence and human judgment at the right
points. Shaping is interactive because product meaning and scope need agreement.
Task execution is autonomous because implementation work can be delegated.
Walking is interactive because the result must be shown as behavior, not only as
a diff. Review and merge connect the proven behavior back to the repository and
roadmap.

## Ten Workflow Responsibilities

1. Issue intake and backlog curation
2. Shape spec
3. Orchestrate execution
4. Walk spec
5. Gate check
6. Handle feedback
7. Merge conflict resolution
8. Dependency management
9. Scope creep control
10. Lifecycle state persistence and transitions

## Orchestration Loop

Per task loop:

1. `do-task`
2. gate check
3. `audit-task`
4. `investigate` records root cause when needed
5. `do-task` repairs the same task when needed

After all tasks:

1. spec gate (full suite)
2. `run-demo`
3. `audit-spec`

### Runtime Safety Rules

- **Staleness detection**: abort when plan progress does not advance for three
  consecutive cycles.
- **Retry handling**: transient failures back off at 10s/30s/60s; fatal errors
  fail fast.
- **Scope guard**:
  - required implication of spec -> allowed
  - small drive-by fix -> allowed with PR note
  - otherwise -> defer to a new intake issue

## `run-demo` Expectations

`run-demo` verifies behavior, not code style. It should exercise real product
flows (UI/API/data behavior) and report evidence back to the workflow.

## Related Specs

- Store protocols and dispatch lifecycle: `../protocol/README.md`
- Orchestration state machine: `../architecture/orchestration-flow.md`
- Runtime implementation: `../ADAPTERS.md`
