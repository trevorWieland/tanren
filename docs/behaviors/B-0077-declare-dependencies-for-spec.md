---
schema: tanren.behavior.v0
id: B-0077
title: Declare dependencies for a spec
area: spec-lifecycle
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can declare dependencies for a spec so that Tanren does not start work before required foundation work is complete.

## Preconditions

- The spec exists and is visible to the user.
- The user has permission to edit dependencies for the spec.

## Observable outcomes

- The spec records dependencies on other specs or allowed external references.
- The user can see unresolved dependencies from the spec view.
- Loop-start behavior honors the declared dependencies.

## Out of scope

- Inferring undeclared dependencies automatically.
- Overriding dependency blocks.

## Related

- B-0017
- B-0029
- B-0056
