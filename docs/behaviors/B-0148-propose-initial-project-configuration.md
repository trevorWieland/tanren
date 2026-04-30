---
schema: tanren.behavior.v0
id: B-0148
title: Review initial project configuration proposals from repo source signals
area: repo-understanding
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can review initial project configuration
proposals derived from repository source signals so setup reflects how the project
already works without applying inferred settings blindly.

## Preconditions

- Repository analysis has produced configuration source signals.
- The user has permission to manage project configuration.

## Observable outcomes

- Tanren proposes methodology, command, verification, runtime, or standards
  settings with source signals.
- The user can accept, revise, or reject each proposed setting.
- Accepted configuration changes are recorded with attribution.

## Out of scope

- Applying inferred configuration without review when policy requires review.
- Encoding repository-specific implementation details in behavior docs.
- Replacing the underlying configuration behaviors that own each accepted
  setting.

## Related

- B-0049
- B-0087
- B-0146
