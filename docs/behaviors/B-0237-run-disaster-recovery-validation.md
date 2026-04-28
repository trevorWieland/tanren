---
id: B-0237
title: Run disaster recovery validation
area: operations
personas: [operator, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can run disaster recovery validation so backups and recovery paths are trusted before they are needed.

## Preconditions

- Backup, export, restore, or recovery configuration exists for the selected scope.
- The user has permission to validate recovery readiness.

## Observable outcomes

- Tanren reports whether required recovery artifacts, permissions, and procedures are present.
- Validation failures are actionable without exposing secret values.
- Results are stored as operational evidence.

## Out of scope

- Performing destructive restore into production by default.
- Guaranteeing recovery for resources outside configured scope.

## Related

- B-0063
- B-0064
- B-0242
