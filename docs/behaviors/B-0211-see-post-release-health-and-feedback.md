---
schema: tanren.behavior.v0
id: B-0211
title: See post-release health and feedback
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see post-release health and feedback so delivered work can be evaluated after it reaches users.

## Preconditions

- Shipped work or a release exists.
- Tanren has post-release checks, feedback, support notes, complaints, metrics, or observations visible to the user.

## Observable outcomes

- Tanren links post-release signals to the shipped work they concern.
- Signals distinguish healthy, degraded, missing, mixed, and needs-follow-up states.
- Follow-up candidates can be traced back to their post-release source signals.

## Out of scope

- Inferring production health without source signals.
- Importing private user data without configured consent and scope.

## Related

- B-0179
- B-0181
- B-0182
