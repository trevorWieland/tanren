---
id: B-0175
title: Revert landed work through controlled follow-up
area: undo-recovery
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can revert landed work through controlled follow-up so shipped changes can be undone without losing traceability.

## Preconditions

- Work has landed in the project repository or release stream.
- The user has permission to create or approve revert work for the project.

## Observable outcomes

- Tanren creates or routes revert work linked to the original spec, evidence, and merge.
- The revert follows normal shaping, execution, review, and merge policy.
- The original work remains historically visible after the revert lands.

## Out of scope

- Silently deleting repository history.
- Treating a revert as proof that the original decision was invalid.

## Related

- B-0119
- B-0121
- B-0180
