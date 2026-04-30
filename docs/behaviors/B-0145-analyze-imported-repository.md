---
schema: tanren.behavior.v0
id: B-0145
title: Analyze an imported repository before planning work
area: repo-understanding
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can analyze an imported repository before planning work so Tanren understands the existing project context.

## Preconditions

- An active project is connected to an existing repository.
- The user has permission to inspect repository contents.

## Observable outcomes

- Tanren summarizes repository structure, major surfaces, and detected conventions.
- Unknown or ambiguous parts of the repository are called out for user review.
- The analysis can support configuration, standards, and roadmap planning.

## Out of scope

- Changing repository files during analysis.
- Claiming architectural certainty where source signals are missing.

## Related

- B-0025
- B-0146
- B-0147
