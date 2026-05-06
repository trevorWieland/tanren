---
name: craft-roadmap
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
  - docs/roadmap/dag.json
  - docs/roadmap/roadmap.md
---

# craft-roadmap

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes roadmap
projections directly because native roadmap DAG schemas, graph validators,
typed tools, and project-method events do not exist yet. Prefer structured
JSON, explicit dependency edges, stable IDs, and small approved edits so these
artifacts can later migrate into typed Tanren storage.

This command is for any repository adopting the Tanren method. Use the
repository's configured roadmap artifact paths; if none are configured, use the
conventional `docs/roadmap/` paths.

## Purpose

Turn accepted behaviors, accepted architecture, and current implementation
state into a dependency-aware roadmap DAG of spec-sized work. The DAG is the
planning source of truth; the human roadmap is a rendered explanation of that
graph.

## Inputs

- Product projections from `docs/product/**`.
- Accepted behavior catalog from `docs/behaviors/**`.
- Surface registry and experience projections from `docs/experience/**`.
- Architecture projections from `docs/architecture/**`.
- Implementation-readiness and verification projections from
  `docs/implementation/**`.
- Current roadmap DAG and human-readable roadmap, if present.
- Completed, in-flight, blocked, or planned specs, if available.
- Bug reports, client requests, support feedback, audit findings, or proactive
  analysis reports supplied by the user.

## Editable Artifacts

This command owns:

- `docs/roadmap/dag.json`
- `docs/roadmap/roadmap.md`

## Temporary DAG Format

```json
{
  "schema": "tanren.roadmap_dag.v0",
  "generated_at": "YYYY-MM-DD",
  "product_ref": "docs/product/vision.md",
  "behavior_root": "docs/behaviors",
  "surface_registry_ref": "docs/experience/surfaces.yml",
  "experience_root": "docs/experience",
  "architecture_root": "docs/architecture",
  "implementation_ref": "docs/implementation/readiness.json",
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
      "surface_scope": ["web", "cli"],
      "experience_risk": "medium",
      "expected_evidence": [
        {
          "kind": "bdd",
          "behavior_id": "B-0140",
          "surfaces": ["web", "cli"],
          "witnesses": ["positive", "falsification"],
          "description": "Positive and falsification scenarios assert product brief creation"
        }
      ],
      "scope": "Implement a temporary plan-product command and artifact format",
      "rationale": "Planning needs durable product intent before behavior identification can be complete",
      "risks": [],
      "shape_notes": []
    }
  ]
}
```

## DAG Rules

- Every executable node must complete at least one accepted behavior.
- `supports_behaviors` may add context but cannot replace
  `completes_behaviors`.
- Every node belongs to exactly one milestone.
- Dependency edges must be explicit and acyclic.
- `surface_scope`, when present, must use IDs from
  `docs/experience/surfaces.yml`.
- `experience_risk`, when present, must be `low`, `medium`, or `high`.
- `expected_evidence[].surfaces` should match the completed behavior's declared
  surfaces. Existing `interfaces` entries are accepted as a migration alias.
- Nodes should be small enough to shape, orchestrate, walk, review, and merge
  independently.
- Completed and in-flight nodes should be preserved during replanning.
- New feedback should revise the graph without erasing history.

## Responsibilities

1. Read product intent, accepted behaviors, surfaces, experience contracts,
   architecture, implementation state, existing roadmap artifacts, and
   in-flight work.
2. Classify bugs, feedback, or analysis as missing behavior, misaligned
   behavior, implementation gap, evidence gap, architecture gap, roadmap
   dependency change, priority change, false report, or out-of-scope report.
3. Propose milestones and graph-shaping assumptions before editing.
4. Draft or revise the DAG with stable node IDs, explicit edges, surface scope,
   experience risk, and expected evidence.
5. Verify manually that every executable node completes at least one accepted
   behavior.
6. Verify manually that dependencies are acyclic.
7. Render or update the human-readable roadmap from the DAG.
8. Summarize added nodes, changed dependencies, preserved in-flight work,
   unresolved decisions, and evidence gaps.

## Out of Scope

- Editing product docs. Use `plan-product`.
- Defining project surface IDs. Use `define-surfaces`.
- Editing behavior docs or behavior status. Use `identify-behaviors`.
- Designing behavior-surface flows and states. Use `design-experience`.
- Choosing or revising architecture. Use `architect-system`.
- Assessing current implementation state. Use `assess-implementation`.
- Dispatching specs, creating tasks, opening pull requests, or mutating
  orchestration state.
