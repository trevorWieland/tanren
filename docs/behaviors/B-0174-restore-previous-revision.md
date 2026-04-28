---
id: B-0174
title: Restore a previous revision without deleting history
area: undo-recovery
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can restore a previous revision of a supported Tanren artifact so
accidental or harmful changes can be corrected without deleting later history.

## Preconditions

- A prior revision exists for a supported artifact type.
- The user has permission to restore the affected scope.

## Observable outcomes

- Tanren previews the restoration impact before applying it when policy requires review.
- The restored revision becomes active without deleting later history.
- The restoration is attributed and linked to the prior revision.

## Out of scope

- Restoring secrets by showing secret values.
- Rewriting immutable audit history.
- Treating every bad product decision as a simple restore.

## Related

- B-0089
- B-0114
- B-0177
