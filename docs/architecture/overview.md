# Architecture Overview

Last updated: April 24, 2026

## What Tanren Is

Tanren is a product-to-proof control plane for agentic software delivery. It
combines three first-class systems:

1. **Product method**: product briefs, accepted behavior catalogs, roadmap
   DAGs, proactive analysis findings, and behavior evidence keep work tied to
   product intent.
2. **Execution framework**: the worker daemon provisions environments, routes
   dispatches, runs phases, handles retries, and records outcomes.
3. **Methodology system**: command files, standards profiles, and product
   templates define how agents shape specs, implement, audit, and validate work.

The framework without product method runs quickly but drifts. Product method
without execution stays aspirational. Proactive analysis without behavior and
roadmap routing creates noise instead of planned progress. Tanren couples
product intent, behavior contracts, roadmap sequencing, scheduled discovery,
and typed execution to deliver repeatable, automatable software delivery.

The central boundary is simple: Tanren decides what work exists, why it exists,
what phase may run next, and what evidence is required. Agent runtimes decide
how one assigned role reasons and edits within those boundaries.

## Three-Layer Model

```
Coordinator -> Tanren -> Agent Runtime
```

- **Coordinator (above tanren)**: identity, authorization, metering views,
  and human interface (dashboard/CLI/chat).
- **Tanren (this repo)**: product method, behavior canon, roadmap DAG,
  issue/spec lifecycle, dependency orchestration, gate checks, feedback loop,
  evidence, and environment management.
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

- Orchestration state machine: [orchestration-flow](orchestration-flow.md)
- Tool and CLI fallback contract: [agent-tool-surface](agent-tool-surface.md)
- Runtime implementation details: [ADAPTERS](../ADAPTERS.md)
- Evidence schemas: [evidence-schemas](evidence-schemas.md)
