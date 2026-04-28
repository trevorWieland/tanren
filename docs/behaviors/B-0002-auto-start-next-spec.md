---
id: B-0002
title: Automatically start eligible ready work when serial execution is configured
area: implementation-loop
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can configure Tanren to automatically start an
implementation loop on eligible ready work when another loop finishes, so that
independent shaped work can continue without manual starts while still respecting
planning boundaries.

## Preconditions

- An active project is selected.
- The user can configure automatic triggers for the project (personal context)
  or within their team's project configuration (organizational context).
- One or more specs are already shaped, ready, and eligible under the project's
  sequencing, dependency, budget, placement, and autonomy rules.

## Observable outcomes

- When a configured trigger fires, an eligible ready spec's loop starts on its
  own.
- The automatic start is visible: the user can see which trigger started it and
  why that spec was eligible.
- The user can turn automatic triggers off without affecting loops already
  running.

## Out of scope

- Creating or shaping follow-up specs automatically.
- Choosing *which* eligible spec runs next — ordering and prioritization are
  covered elsewhere.
- Scheduled or time-based triggers — only completion-based triggers here.
- Cross-project triggers.

## Related

- B-0001
- B-0003
- B-0004
