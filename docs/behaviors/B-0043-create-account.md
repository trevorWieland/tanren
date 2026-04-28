---
id: B-0043
title: Create an account
area: governance
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A person can create a Tanren account, either by self-signup for a personal
account or by accepting an invitation from an existing organization member,
so that they can sign in and start using Tanren.

## Preconditions

- For self-signup: the person is not already signed into Tanren with the
  identifier they want to use.
- For invitation-based creation: the person has received a valid invitation
  from someone with permission to invite into an organization (B-0044).

## Observable outcomes

- After creation the account can be signed into from any supported
  interface, including on a phone.
- A self-signed-up account starts not belonging to any organization
  (personal).
- An account created from an invitation is joined to the inviting
  organization upon creation (see B-0045).
- A single person can create or hold multiple accounts (for example, a
  personal account and a work account).

## Out of scope

- Identity provider selection, SSO, passwordless, and other
  authentication-mechanism specifics.
- Account deletion or transfer.

## Related

- B-0044
- B-0045
