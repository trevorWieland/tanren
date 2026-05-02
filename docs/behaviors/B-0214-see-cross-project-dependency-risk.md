---
schema: tanren.behavior.v0
id: B-0214
title: See cross-project dependency risk
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see cross-project dependency risk so project plans account for work outside a single project boundary.

## Preconditions

- The selected scope includes multiple projects or dependencies that reference other projects.
- The user has visibility into at least the dependency relationship.

## Observable outcomes

- Tanren shows cross-project dependencies, blocked work, and risk to dependent roadmap items or specs.
- Hidden dependency details are redacted without hiding that a dependency exists.
- The user can navigate to visible blocking or dependent work.

## Out of scope

- Granting access to projects through dependency visibility.
- Forcing all projects into one roadmap.

## Related

- B-0029
- B-0111
- B-0206
