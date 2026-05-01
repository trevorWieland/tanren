---
schema: tanren.behavior.v0
id: B-0045
title: Join an organization with an existing account
area: governance
personas: [solo-builder, team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A person with an existing Tanren account can accept an invitation (B-0044)
to join an organization, so that a standalone or previously work-specific
account can be attached to an additional organization without having to
create a new account.

## Preconditions

- The person holds an existing account.
- They have received a valid invitation (B-0044) addressed to that account
  or its identifier.

## Observable outcomes

- After acceptance, the account is a member of the inviting organization
  with the organization-level permissions granted on the invitation.
  Project access within the organization is not granted by acceptance; it
  is assigned separately per project via B-0031.
- The account's membership in any other organizations is unaffected.
- The user can switch into the newly joined organization via B-0047.
- The user can leave the organization later via B-0059, at which point
  their access to the organization's projects ends but their account
  remains.

## Out of scope

- Merging two accounts into one.
- Automatic assignment to an organization based on email domain.

## Related

- B-0043
- B-0044
- B-0047
