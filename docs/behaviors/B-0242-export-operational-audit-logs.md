---
schema: tanren.behavior.v0
id: B-0242
title: Export operational audit logs
area: operations
personas: [observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can export operational audit logs so governance, incident review, and compliance workflows can inspect Tanren-controlled activity.

## Preconditions

- The user has permission to export audit logs for the selected scope.
- Audit records exist for that scope.

## Observable outcomes

- Exported logs include attributed operational, policy, credential, integration, worker, and provider events.
- Redactions and omitted scopes are visible in the export.
- Export creation is itself audited.

## Out of scope

- Exporting secret values.
- Granting broader visibility than the user has inside Tanren.

## Related

- B-0042
- B-0133
- B-0229
