---
id: B-0187
title: See shared work that needs my attention
area: team-coordination
personas: [team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can see shared work that needs their attention so blockers, reviews, approvals, and handoffs do not get lost.

## Preconditions

- The user has access to a shared project.
- One or more visible work items need action, response, or review.

## Observable outcomes

- Tanren identifies work waiting on the user or on a group the user belongs to.
- Each attention item explains the needed action at a user level.
- Items outside the user's visible scope are omitted or redacted according to policy.

## Out of scope

- Assigning blame for delayed work.
- Assuming the user has permission to resolve every visible attention item.

## Related

- B-0004
- B-0188
- B-0195
