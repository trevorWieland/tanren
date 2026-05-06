---
name: identify-behaviors
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
  - docs/behaviors/B-*.md
  - docs/behaviors/index.md
---

# identify-behaviors

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes behavior
projections directly because native behavior-catalog schemas, typed tools, and
project-method events do not exist yet. Prefer structured frontmatter, stable
IDs, explicit rationale, and small approved edits so these artifacts can later
migrate into typed Tanren storage.

This command is for any repository adopting the Tanren method. Use the
repository's configured behavior catalog path; if none is configured, use the
conventional `docs/behaviors/` path.

## Purpose

Turn product intent into a durable behavior canon and maintain each behavior's
product and verification status over time.

A behavior is a high-level user, client, operator, or runtime-actor capability.
It describes what an actor can accomplish and what outcome is observable. It
does not describe implementation internals.

Behavior files are portable product contracts. Product docs and behavior files
should remain useful if copied into a fresh repository and implemented with a
different language, architecture, runtime, or test suite.

## Inputs

- `docs/product/vision.md`, `docs/product/personas.md`, and
  `docs/product/concepts.md`.
- Existing behavior files and `docs/behaviors/index.md`.
- Surface IDs from `docs/experience/surfaces.yml`.
- Runtime actor IDs from architecture docs.
- Implementation and verification projections, if present.
- User feedback, bug reports, client requests, audit findings, or planning
  notes supplied by the user.

## Editable Artifacts

This command owns:

- `docs/behaviors/B-*.md`
- `docs/behaviors/index.md`

This command may create behavior files, update behavior frontmatter, update
product status, update verification status when evidence supports it, add
`supersedes` links, deprecate or remove behavior IDs with rationale, and update
the catalog index.

## Temporary Artifact Format

```markdown
---
schema: tanren.behavior.v0
id: B-0001
title: <imperative user-visible capability>
area: <stable area>
personas: []
runtime_actors: []
surfaces: []
contexts: []
product_status: draft | accepted | deprecated | removed
verification_status: unimplemented | implemented | asserted | retired
supersedes: []
---

## Intent
## Preconditions
## Observable Outcomes
## Out of Scope
## Related
```

## Responsibilities

1. Read product intent and current behavior coverage before proposing edits.
2. Identify missing behaviors, overlapping behaviors, oversized behaviors,
   implementation-shaped behaviors, and stale status.
3. Propose additions and revisions in a reviewable batch before changing files.
4. Create behavior files for accepted additions using stable IDs.
5. Update `product_status` only with product rationale.
6. Update `verification_status` only when implementation or executable evidence
   supports it; summarize evidence in implementation projections rather than
   embedding implementation references in behavior files.
7. Use `implemented` when code appears to support the behavior but active
   executable behavior evidence is missing.
8. Use `asserted` only when active BDD evidence exists.
9. Deprecate or remove accepted behavior IDs instead of silently repurposing
   them.
10. Keep `interfaces:` only as a migration alias for existing Tanren behavior
    files; new adopting projects should write `surfaces:` IDs from the active
    surface registry.
11. Summarize added behaviors, revised behaviors, status changes, unresolved
    decisions, and evidence gaps.

## Out of Scope

- Editing product vision, personas, or concepts. Use `plan-product`.
- Choosing implementation architecture. Use `architect-system`.
- Creating roadmap DAG nodes. Use `craft-roadmap`.
- Writing implementation code.
- Dispatching specs, creating tasks, opening pull requests, or mutating
  orchestration state.
