---
schema: tanren.behavior.v0
id: B-0046
title: Switch the active account
area: governance
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A person can switch between accounts they hold — for example between a
personal account and a work account — from within the same interface, so
that they can move between distinct sets of projects and organizations
without fully signing out.

## Preconditions

- The user is signed into at least one account.
- The user has credentials for each account they want to switch between.

## Observable outcomes

- The user can list every account they are currently signed into and select
  any of them as the active account.
- Switching accounts changes which projects and organizations are available
  and which configuration applies.
- The user can run Tanren in multiple windows at the same time — for
  example a personal account in one window and a work account in another —
  with each window showing work against its own active account.
- Switching is equally easy on a phone as on a laptop.
- The state of the previously active account is preserved so that returning
  to it resumes without re-authentication.

## Out of scope

- A single combined view that mixes projects, loops, or specs from
  multiple accounts — each window is scoped to one active account.
- Sharing any state, selection, or preference across accounts.

## Related

- B-0028
- B-0043
- B-0047
