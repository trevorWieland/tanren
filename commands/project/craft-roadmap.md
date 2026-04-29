---
name: craft-roadmap
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
  - roadmap-dag.json
  - ROADMAP.md
  - roadmap synthesis report
---

# craft-roadmap

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes roadmap
artifacts directly because native roadmap DAG schemas, graph validators,
tools, and project-state events do not exist yet. Prefer structured
JSON, explicit dependency edges, stable IDs, and small approved edits
so these artifacts can later migrate into typed Tanren storage.

This command is for any repository adopting the Tanren method. When it
is used in the Tanren repository, use Tanren's local roadmap docs as
the configured roadmap artifacts. Do not assume every repository has
the same file layout.

## Purpose

Turn accepted behaviors and current implementation state into a
dependency-aware roadmap DAG of spec-sized work. The DAG is the
planning source of truth; the human roadmap is a rendered explanation
of that graph.

## Inputs

- Product brief, vision, motivations, constraints, and non-goals.
- Accepted behavior catalog and verification status.
- Current roadmap DAG and human-readable roadmap, if present.
- Implementation-readiness reports, if present.
- Current code, tests, docs, and architecture context as needed.
- Completed, in-flight, blocked, or planned specs, if available.
- Bug reports, client requests, support feedback, audit findings, or
  proactive analysis reports supplied by the user.

## Editable Artifacts

Use the repository's configured roadmap location. If none is
configured, infer the conventional location and confirm it with the
user before editing.

This command may create or revise:

- `roadmap-dag.json`;
- `ROADMAP.md`;
- a roadmap synthesis report.

## Temporary DAG Format

Prefer this temporary DAG shape:

```json
{
  "schema": "tanren.roadmap_dag.v0",
  "generated_at": "YYYY-MM-DD",
  "product_ref": "docs/product/vision.md",
  "behavior_root": "docs/behaviors",
  "milestones": [
    {
      "id": "M-0001",
      "title": "Bootstrap product method",
      "goal": "Make product planning artifacts durable and behavior-backed",
      "status": "planned"
    }
  ],
  "nodes": [
    {
      "id": "R-0001",
      "title": "Create temporary product planning command",
      "status": "planned",
      "milestone": "M-0001",
      "completes_behaviors": ["B-0140"],
      "supports_behaviors": [],
      "depends_on": [],
      "expected_evidence": [
        {
          "kind": "bdd",
          "behavior_id": "B-0140",
          "description": "Positive and falsification scenarios assert product brief creation"
        }
      ],
      "scope": "Implement a temporary plan-product command and artifact format",
      "rationale": "Planning needs a durable product brief before behavior identification can be complete",
      "risks": [],
      "shape_notes": []
    }
  ]
}
```

Prefer this human-readable roadmap shape:

```markdown
---
schema: tanren.roadmap_view.v0
source: roadmap-dag.json
updated_at: YYYY-MM-DD
---

# Roadmap

## Milestone: <title>

### <node id>: <title>

- Status:
- Completes behaviors:
- Depends on:
- Evidence:
- Rationale:
```

## DAG Rules

- Every executable node must complete at least one accepted behavior.
- `supports_behaviors` may add context but cannot replace
  `completes_behaviors`.
- Every node belongs to exactly one milestone.
- Dependency edges must be explicit and acyclic.
- Nodes should be small enough to shape, orchestrate, walk, review,
  and merge independently.
- Completed and in-flight nodes should be preserved during replanning.
- New feedback should revise the graph without erasing history.

## Responsibilities

1. Identify the repository's roadmap artifact location and confirm it
   with the user if ambiguous.
2. Read product intent, accepted behaviors, verification status,
   readiness reports, existing roadmap artifacts, and in-flight work.
3. Classify inputs from bugs, feedback, or analysis as missing
   behavior, misaligned behavior, implementation gap, evidence gap,
   roadmap dependency change, priority change, false report, or
   out-of-scope report.
4. Propose milestones and graph-shaping assumptions before editing.
5. Draft or revise the DAG with stable node IDs and explicit edges.
6. Verify manually that every executable node completes at least one
   accepted behavior.
7. Verify manually that dependencies are acyclic.
8. Render or update the human-readable roadmap from the DAG.
9. Summarize added nodes, changed dependencies, preserved in-flight
   work, unresolved decisions, and evidence gaps.

## Out of Scope

- Creating or revising product brief or persona docs. Use
  `plan-product`.
- Creating or revising behavior files or behavior status. Use
  `identify-behaviors`.
- Dispatching specs, creating tasks, opening pull requests, or
  mutating orchestration state.
- Claiming BDD assertion when evidence does not exist.
