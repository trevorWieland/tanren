---
schema: tanren.behavior.v0
id: B-0215
title: Export a read-only status report
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can export a read-only status report so project progress and risk can be shared without granting edit access.

## Preconditions

- The user has visibility into the report scope.
- The selected scope has status information to report.

## Observable outcomes

- The report includes progress, blockers, risks, recent outcomes, and source references appropriate to the user's visible scope.
- Exported content is read-only and does not grant additional Tanren permissions.
- Redactions and data freshness are visible in the report.

## Out of scope

- Publishing reports to external systems without configured permission.
- Exporting secret values or hidden project details.

## Related

- B-0037
- B-0204
- B-0209
