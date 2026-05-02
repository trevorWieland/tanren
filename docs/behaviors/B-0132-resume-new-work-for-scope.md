---
schema: tanren.behavior.v0
id: B-0132
title: Resume new work for a project or organization
area: operations
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with permission can resume new Tanren work for a paused project or organization so normal execution can continue after a stop condition is resolved.

## Preconditions

- New work is paused for the scope.
- The user has permission to resume work intake.

## Observable outcomes

- The pause is lifted for the scope.
- Eligible queued or future work can start again according to policy.
- The resume action is attributed and visible.

## Out of scope

- Bypassing unresolved policy blocks.
- Automatically rerunning cancelled work.

## Related

- B-0131
- B-0130
