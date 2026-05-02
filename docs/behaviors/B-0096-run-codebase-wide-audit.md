---
schema: tanren.behavior.v0
id: B-0096
title: Run a codebase-wide audit
area: operations
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can run an audit across a codebase so broad risks can be found outside a single spec loop.

## Preconditions

- A project repository is connected or bootstrapped.
- The user has permission to run broad audits.

## Observable outcomes

- The audit records scope, source signals, and findings.
- Findings are visible after the audit completes.
- The audit does not silently mutate product work without user approval.

## Out of scope

- Replacing task- or spec-scoped checks.
- Automatically fixing every finding.

## Related

- B-0080
- B-0097
