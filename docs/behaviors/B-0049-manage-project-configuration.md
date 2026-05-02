---
schema: tanren.behavior.v0
id: B-0049
title: Manage project methodology settings
area: configuration
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with the required permission can manage the
methodology settings for a project so that the project's methodology profile,
standards, and local working rules are consistent for everyone with access.

## Preconditions

- The user has permission to change project configuration for the active
  project. In organizational contexts this permission may be restricted by
  organization policy.

## Observable outcomes

- The user can view the project's active methodology profile, the methodology
  commands available to the project, the standards in effect, and local
  working rules.
- The user can change project methodology settings when policy allows it.
- Methodology configuration applies to subsequent project work.
- Changes are attributed and visible in the project's change history.

## Out of scope

- Runtime defaults.
- Verification gates.
- Project-scoped secrets.
- Organization standards profile lifecycle and requirements.
- Automatic migration of configuration between projects.

## Related

- B-0048
- B-0050
- B-0051
- B-0086
- B-0087
- B-0088
- B-0089
