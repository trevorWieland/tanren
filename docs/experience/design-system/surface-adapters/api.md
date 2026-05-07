---
schema: tanren.design_surface_adapter.v0
surface_kind: machine_contract
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# API Adapter

The API adapter projects shared design-system records into HTTP resources,
schemas, response bodies, error envelopes, pagination, freshness, and examples.

## Projection

- Vocabulary terms map to stable response codes and schema fields.
- `output.machine.error-envelope` is the default error pattern.
- Surface tokens do not imply visual style; they map to semantic response
  roles such as status, detail, warning, and recovery hint.
- Examples should use accepted behavior and product vocabulary.

## Required Evidence

- Contract BDD against real request and response paths.
- OpenAPI schema evidence.
- Positive and falsification examples for machine-readable errors.

## Drift Signals

- New error codes not registered in vocabulary.
- Handwritten response examples that disagree with schema.
- Human-only wording where clients need machine-readable detail.
