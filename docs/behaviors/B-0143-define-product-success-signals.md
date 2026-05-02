---
schema: tanren.behavior.v0
id: B-0143
title: Define product success signals
area: product-discovery
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can define product success signals so Tanren can relate roadmap and shipped work to desired outcomes.

## Preconditions

- An active project is selected.
- The user has permission to edit product planning context.

## Observable outcomes

- Success signals are recorded with the product context they measure.
- Roadmap items and shipped changes can link to relevant signals.
- Signals can be revised without erasing prior versions.

## Out of scope

- Automatically proving business success.
- Ingesting external analytics without a configured source.

## Related

- B-0092
- B-0098
- B-0182
