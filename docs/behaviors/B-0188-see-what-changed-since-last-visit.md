---
schema: tanren.behavior.v0
id: B-0188
title: See what changed since I last visited the project
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see what changed since they last visited a project so shared work can be re-entered without losing context.

## Preconditions

- The user has access to the project.
- Tanren has recorded project changes after the user's prior visit or chosen comparison point.

## Observable outcomes

- Tanren summarizes relevant changes to roadmap, specs, graph, active work, decisions, standards, reviews, and releases.
- The user can filter changes by project area or attention relevance.
- Hidden changes are redacted without implying nothing changed.

## Out of scope

- Showing private details from scopes the user cannot see.
- Replacing full audit history.

## Related

- B-0014
- B-0042
- B-0187
