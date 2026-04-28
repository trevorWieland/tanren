---
id: B-0233
title: Configure credential use policy by scope
area: governance
personas: [operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can configure credential use policy by scope so Tanren knows which access material may be used for each kind of work.

## Preconditions

- The user has permission to configure credential policy for the selected scope.

## Observable outcomes

- Policy can distinguish user-owned, project-owned, organization-owned, service-account, and worker-scoped access.
- Work that requests disallowed access is blocked with an explainable policy denial.
- Policy changes are attributed, reviewable, and testable before application where required.

## Out of scope

- Storing secret values in policy text.
- Making personas the basis of credential authorization.

## Related

- B-0163
- B-0238
- B-0239
