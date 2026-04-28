---
id: B-0236
title: Enter maintenance or incident mode
area: operations
personas: [operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can enter maintenance or incident mode so Tanren-controlled work follows a safer operating posture during disruption.

## Preconditions

- The user has permission to change operational mode for the selected scope.

## Observable outcomes

- New work, active work, approvals, worker access, and notifications follow the selected mode policy.
- Users with visibility can see the mode, reason, scope, and expected handling.
- Leaving the mode is attributed and resumes normal policy deliberately.

## Out of scope

- Hiding incidents from affected users.
- Treating incident mode as permission to bypass audit.

## Related

- B-0131
- B-0132
- B-0235
