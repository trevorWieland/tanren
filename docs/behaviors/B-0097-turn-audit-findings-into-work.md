---
id: B-0097
title: Turn audit findings into specs or backlog items
area: intake
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can turn broad audit findings into specs or backlog items so remediation becomes planned work.

## Preconditions

- An audit has produced findings.
- The user has permission to create or update specs.

## Observable outcomes

- The user can accept a finding into candidate work.
- Created specs or backlog items link back to the audit finding.
- Rejected findings remain recorded with disposition.

## Out of scope

- Automatically accepting all findings.
- Hiding rejected findings from audit history.

## Related

- B-0018
- B-0020
- B-0096
