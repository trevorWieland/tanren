# Tanren Roadmap

The roadmap is the dependency-aware plan that connects accepted behaviors to
spec-sized implementation work.

Tanren's eventual roadmap source of truth is a machine-readable DAG:

```text
docs/roadmap/roadmap-dag.json
```

`ROADMAP.md` is the human-readable rendering of that graph. It should explain
milestones, sequencing, and progress for people, but it should not be the
durable planning database once the DAG exists.

## Roadmap Role

The roadmap sits between the behavior catalog and the spec loop:

```text
accepted behaviors -> roadmap DAG -> shaped specs -> BDD evidence
```

It answers:

- which accepted behaviors still need implementation or assertion;
- which spec-sized nodes complete those behaviors;
- which nodes depend on other nodes;
- which milestone each node belongs to;
- what evidence should prove completion;
- how new feedback or analysis changes the plan without erasing history.

## Node Rules

Every executable roadmap node must:

- complete at least one accepted behavior;
- declare the behavior evidence it is expected to assert;
- belong to exactly one milestone;
- use explicit dependency edges;
- be small enough to shape, orchestrate, walk, review, and merge independently;
- preserve completed and in-flight work when the graph is revised.

If a proposed node completes no behavior, it is either too thin, too internal,
or missing the behavior that explains why it matters.

## Inputs

Roadmap synthesis consumes:

- accepted behaviors and their verification status;
- implementation-readiness reports;
- current code, tests, docs, and architecture;
- completed, in-flight, and blocked specs;
- bug reports, client requests, and support feedback;
- proactive analyses such as standards sweeps, security audits,
  mutation-testing reports, dependency audits, and post-ship health checks.

Those inputs usually become one of:

- a missing behavior to add;
- a correction to an existing behavior;
- a gap in implementation or executable evidence;
- a new roadmap node;
- a dependency, milestone, or priority change;
- a false or out-of-scope report.

## Status

The roadmap DAG command and `roadmap-dag.json` format are planned, not yet
implemented. Until then, this directory documents the target shape of roadmap
work and `ROADMAP.md` remains a human-readable planning view.
