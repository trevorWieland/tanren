---
schema: tanren.behavior.v0
id: B-0086
title: Manage project runtime defaults
area: configuration
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with permission can configure default runtime preferences for a project so new work starts in an appropriate execution context.

## Preconditions

- An active project is selected.
- The user has permission to manage runtime defaults.

## Observable outcomes

- The project records default runtime preferences.
- New work uses the defaults unless an allowed override applies.
- Users can see the effective runtime defaults before starting work.

## Out of scope

- Provisioning execution targets.
- Bypassing organization placement policy.

## Related

- B-0049
- B-0081
- B-0102
