---
schema: tanren.behavior.v0
id: B-0270
title: Configure what counts as shipped for a project
area: release-learning
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can configure what counts as shipped for a project so release, learning,
and outcome views match the project's delivery model instead of assuming merge
is always release.

## Preconditions

- The project exists.
- The user has permission to manage release or project configuration.

## Observable outcomes

- The project records whether shipped means merged, deployed, released,
  manually marked, or confirmed by an external signal.
- Recently shipped outcome views use the configured definition.
- Missing or unavailable release source signals are visible instead of treated as
  success.
- Changes to the shipped definition are attributed and affect subsequent
  reporting.

## Out of scope

- Requiring every project to have the same deployment model.
- Publishing release notes or external announcements.
- Treating shipped status as proof of product success.

## Related

- B-0178
- B-0179
- B-0180
- B-0210
