# Architecture Overview

Last updated: March 12, 2026

## What Tanren Is

Tanren combines two first-class systems:

1. **Execution framework**: the worker manager provisions environments, routes
   dispatches, runs phases, handles retries, and records outcomes.
2. **Methodology system**: command files, standards profiles, and product
   templates define how agents shape specs, implement, audit, and validate work.

The framework without methodology runs quickly but drifts. Methodology without
framework is high quality but manual. Tanren couples both to deliver repeatable,
automatable software execution.

## Three-Layer Model

```
Coordinator -> Tanren -> Agent Runtime
```

- **Coordinator (above tanren)**: identity, authorization, metering views,
  and human interface (dashboard/CLI/chat).
- **Tanren (this repo)**: issue/spec lifecycle, dependency orchestration,
  gate checks, feedback loop, and environment management.
- **Agent runtime (below tanren)**: role-to-CLI/model routing and auth.

**Boundary rule:** tanren decides what work happens; runtimes decide how each
role is executed.

## Opinionated Core vs Pluggable Integrations

Tanren is opinionated about workflow phases and state transitions. External
systems are adapter-backed and replaceable:

- Issue source: GitHub now, Linear/Jira possible.
- Source control and CI: GitHub-first, other providers possible.
- Execution environments: local subprocess by default, remote VM by adapter.
- Event storage and secrets: file/SQLite defaults with replaceable backends.

## Where To Read Next

- Lifecycle details: `docs/workflow/spec-lifecycle.md`
- Runtime implementation details: `worker-manager/README.md`
- Adapter interfaces: `worker-manager/ADAPTERS.md`
