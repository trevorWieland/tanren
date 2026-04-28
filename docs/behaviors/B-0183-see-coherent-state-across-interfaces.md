---
id: B-0183
title: See coherent Tanren state across public interfaces
area: cross-interface
personas: [solo-builder, team-builder, observer, operator, integration-client]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or integration client can see coherent Tanren state across public interfaces so the interface choice does not create competing truths.

## Preconditions

- The actor can access more than one public Tanren interface for the same scope.
- The actor has visibility into the state being compared.

## Observable outcomes

- Project, spec, loop, graph, configuration, and evidence state agree across public interfaces.
- Interface-specific presentation differences do not change the underlying behavior status.
- Stale or unavailable interface views are marked as such.

## Out of scope

- Requiring every interface to expose every advanced operation.
- Exposing hidden state through a less restricted interface.

## Related

- B-0021
- B-0003
- B-0110
