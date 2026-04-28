---
id: B-0099
title: Add credentials for a code harness
area: runtime-substrate
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can add credentials for a supported code harness so Tanren can run work through the user-approved agent provider.

## Preconditions

- The user is signed into an account.
- The harness is supported by the installation or organization policy.

## Observable outcomes

- The credential is stored without later exposing its secret value.
- The user can see which harness the credential enables.
- Tanren can use the credential only within allowed scope.

## Out of scope

- Choosing a harness for a project.
- Sharing user credentials with teammates.

## Related

- B-0048
- B-0082
- B-0125
