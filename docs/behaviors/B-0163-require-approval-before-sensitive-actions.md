---
id: B-0163
title: Require approval before sensitive actions
area: autonomy-controls
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can require approval before sensitive actions so Tanren does not cross important boundaries automatically.

## Preconditions

- Sensitive actions have been defined by project or organization policy.
- The user has permission to configure or respond to approvals for the scope.

## Observable outcomes

- Approval-gated actions pause before execution.
- The approval request explains the action, risk, scope, and evidence.
- Approval or rejection is attributed and recorded.

## Out of scope

- Defining every possible sensitive action in a persona doc.
- Allowing approval prompts to reveal hidden secrets.

## Related

- B-0115
- B-0162
- B-0165
