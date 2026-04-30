---
schema: tanren.behavior.v0
id: B-0102
title: Start work in an isolated execution environment
area: runtime-substrate
personas: [solo-builder, team-builder, operator, integration-client]
runtime_actors: [agent-worker]
interfaces: [cli, api, mcp, tui, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or automated trigger can start Tanren work in an isolated execution environment so code changes happen within a controlled workspace.

## Preconditions

- The spec or work item is eligible to run.
- A permitted execution environment is available.

## Observable outcomes

- Tanren creates or leases an environment for the work.
- The work is associated with that environment in visible state.
- The environment is constrained by project and organization policy.

## Out of scope

- Selecting implementation tasks manually inside the environment.
- Ignoring runtime placement policy.

## Related

- B-0001
- B-0081
- B-0103
