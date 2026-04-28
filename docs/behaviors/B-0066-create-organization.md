---
id: B-0066
title: Create an organization
area: governance
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can create a new organization and become its initial member, so
that a governance entity exists to own projects, members, and policy.

## Preconditions

- The user is signed into an account.

## Observable outcomes

- A new organization is created and the user's account becomes a member.
- The creator holds the administrative permissions needed to invite
  members (B-0044), manage access (B-0065), configure the organization
  (B-0050), set policy (B-0040), and delete the organization (B-0067).
- The new organization owns no projects initially; projects can be
  connected (B-0025) or created (B-0026) within it afterward.
- The new organization appears in the user's account as an available
  organization and can be selected as active via B-0047.

## Out of scope

- Bulk creation of organizations.
- Seeding a new organization from a template or from another
  organization's configuration.

## Related

- B-0040
- B-0043
- B-0044
- B-0047
- B-0050
- B-0065
- B-0067
