---
schema: tanren.roadmap_view.v0
source: dag.json
status: current
owner_command: craft-roadmap
updated_at: 2026-04-29
---

# Roadmap

This file is the human-readable projection of Tanren's roadmap DAG. The durable
planning source is `dag.json`.

## Current Direction

Tanren is being shaped around the full product-to-proof method:

1. Plan the product vision, personas, concepts, constraints, and success
   signals.
2. Identify accepted behaviors with separate product and verification status.
3. Architect the system that can realize those behaviors.
4. Assess current implementation state against the accepted behavior catalog.
5. Craft a dependency-aware roadmap DAG of spec-sized behavior increments.
6. Execute each node through the shape, orchestrate, walk, review, and merge
   loop.
7. Feed bugs, review feedback, post-ship outcomes, mutation testing, security
   audits, performance findings, and other proactive analyses back into the
   owned planning state.

## Current Source State

`dag.json` is currently skeletal. The next `craft-roadmap` pass should populate
milestones and nodes after consuming:

- `docs/product/**`;
- `docs/behaviors/**`;
- `docs/architecture/**`;
- `docs/implementation/**`;
- current specs, code, tests, behavior proof, and source signals.

## Node Rules

Every executable roadmap node must:

- complete at least one accepted behavior;
- declare expected behavior proof or source references for completed behaviors;
- belong to exactly one milestone;
- use explicit dependency edges;
- be small enough to shape, orchestrate, walk, review, and merge independently;
- preserve completed and in-flight work when the graph is revised.

If a proposed node completes no behavior, it is either too thin, too internal,
or missing the behavior that explains why it matters.
