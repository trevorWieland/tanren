---
schema: tanren.behavior.v0
id: B-0238
title: Test policy changes before applying them
area: governance
personas: [operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can test policy changes before applying them so governance changes do not unexpectedly block or allow work.

## Preconditions

- The user has permission to propose or manage policy for the selected scope.
- A draft policy change exists.

## Observable outcomes

- Tanren previews affected projects, work types, credentials, integrations, and approvals where visible.
- The preview distinguishes newly allowed, newly denied, unchanged, and uncertain outcomes.
- Applying the policy remains a separate attributed action when policy requires it.

## Out of scope

- Proving every future policy interaction.
- Applying policy changes without permission.

## Related

- B-0085
- B-0163
- B-0239
