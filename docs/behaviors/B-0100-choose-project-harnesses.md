---
id: B-0100
title: Choose which harnesses a project may use
area: runtime-substrate
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` with permission can choose which supported
code harnesses a project may use, within any organization allowlist policy, so
execution matches project expectations without violating governance.

## Preconditions

- An active project is selected.
- The user has permission to manage project runtime settings.

## Observable outcomes

- The project records allowed harness choices or a project-specific subset of
  organization-allowed harnesses.
- Policy-blocked harnesses are unavailable with an explanation.
- Future work uses only allowed harnesses.

## Out of scope

- Managing harness credentials.
- Installing new harness support.
- Choosing where work is allowed to run.

## Related

- B-0082
- B-0081
- B-0099
- B-0101
