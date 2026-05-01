---
schema: tanren.behavior.v0
id: B-0219
title: See what changed since the last report
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see what changed since the last report so recurring status reviews focus on meaningful movement.

## Preconditions

- A prior report, digest, or chosen comparison point exists.
- The user has visibility into the selected scope.

## Observable outcomes

- Tanren summarizes changes in progress, risk, blockers, decisions, shipped outcomes, and provenance since the prior report.
- Unchanged areas are distinguishable from areas with missing data.
- The summary links to changed work and supporting source references where visible.

## Out of scope

- Replacing the full project change history.
- Claiming no change when data was unavailable.

## Related

- B-0188
- B-0215
- B-0216
