---
id: B-0002
title: Automatically start the next spec when the current one finishes
personas: [solo-dev, team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can configure Tanren to automatically start an
implementation loop on a next spec when the current loop completes, so that
work progresses without manual intervention between specs.

## Preconditions

- An active project is selected.
- The user can configure automatic triggers for the project (personal context)
  or within their team's project configuration (organizational context).

## Observable outcomes

- When a configured trigger fires, the next spec's loop starts on its own.
- The chain is visible: the user can see that a loop was started automatically
  and which trigger started it.
- The user can turn automatic triggers off without affecting loops already
  running.

## Out of scope

- Choosing *which* spec runs next — ordering and prioritization are covered
  elsewhere.
- Scheduled or time-based triggers — only completion-based triggers here.
- Cross-project triggers.

## Related

- B-0001
- B-0003
- B-0004
