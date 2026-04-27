---
id: B-0039
title: See my own permissions
personas: [solo-dev, team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev`, `team-dev`, or `observer` can see a summary of what they are
permitted to do across the projects and organizations they have access to,
so that they know what is available to them without trial-and-error.

## Preconditions

- The user is signed into an account.

## Observable outcomes

- The user can see, per project and per organization, the list of
  permissions they currently hold.
- For each permission, the user can see how they received it — for
  example an individual grant at the project level (B-0031) or
  organization level (B-0065), or a role template that was applied
  (B-0038). When organization policy (B-0040) constrains or adjusts the
  effective permission, that constraint is also shown as a reason.
- The view is available from any supported interface, including on a phone.

## Out of scope

- Seeing other people's permissions (covered by B-0031 for projects and
  B-0065 for organizations).
- Requesting a permission the user does not have — this behavior is
  informational only.

## Related

- B-0031
- B-0038
- B-0040
- B-0065
