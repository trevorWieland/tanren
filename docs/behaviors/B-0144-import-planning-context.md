---
schema: tanren.behavior.v0
id: B-0144
title: Import existing planning context
area: product-discovery
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can import existing notes, docs, or planning artifacts so Tanren starts from available product context.

## Preconditions

- An active project is selected.
- The user has permission to add product planning context.

## Observable outcomes

- Imported context remains attributable to its source.
- Tanren can propose candidate mission, roadmap, or spec material from the context.
- The user can accept, revise, or reject extracted planning material.

## Out of scope

- Treating imported text as accepted product canon without review.
- Importing secret material into routine product views.

## Related

- B-0079
- B-0092
- B-0095
