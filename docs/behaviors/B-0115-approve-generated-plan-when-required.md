---
schema: tanren.behavior.v0
id: B-0115
title: Approve a generated plan when policy requires approval
area: planner-orchestration
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can approve a generated plan when policy requires review so Tanren does not execute sensitive plans without human consent.

## Preconditions

- A generated plan is waiting for approval.
- The user has permission to approve the plan.

## Observable outcomes

- The user can inspect the plan before approving or rejecting it.
- Approval is recorded and attributed.
- Tanren does not run approval-gated work before approval.

## Out of scope

- Approving work without visibility.
- Bypassing organization approval policy.

## Related

- B-0040
- B-0110
