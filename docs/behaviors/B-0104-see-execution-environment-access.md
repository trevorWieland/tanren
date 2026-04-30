---
schema: tanren.behavior.v0
id: B-0104
title: See what access an execution environment has
area: runtime-substrate
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see the access granted to an execution environment so they understand what Tanren work is allowed to read, write, or request.

## Preconditions

- The user has visibility of the active work or environment.

## Observable outcomes

- The user can see repository, network, credential, approval, and filesystem access at a policy level.
- Restricted access is visible before or during work.
- Secret values remain hidden.

## Out of scope

- Displaying raw secret values.
- Changing access without permission.

## Related

- B-0102
- B-0127
- B-0133
