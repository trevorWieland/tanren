---
name: architect-system
description: Tanren methodology command `architect-system`
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
- docs/architecture/system.md
- docs/architecture/technology.md
- docs/architecture/delivery.md
- docs/architecture/operations.md
- docs/architecture/subsystems/*.md
---

# architect-system

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes architecture
projections directly because native architecture schemas, typed tools, and
project-method events do not exist yet. Prefer structured frontmatter, explicit
decisions, rejected alternatives, and small approved edits so these artifacts
can later migrate into typed Tanren storage.

This command is for any repository adopting the Tanren method. Use the
repository's configured architecture artifact paths; if none are configured,
use the conventional `docs/architecture/` paths.

## Purpose

Turn product intent and accepted behaviors into an implementation strategy that
is concrete enough for roadmap synthesis. This command bridges product planning
and roadmap planning without polluting behavior files with implementation
details.

## Inputs

- Product projections from `docs/product/**`.
- Accepted behavior catalog from `docs/behaviors/**`.
- Existing architecture, technical overview, deployment, operations, API, or
  security docs.
- Current code, tests, build configuration, deployment configuration, and
  repository structure when adopting Tanren in an existing repo.
- Human preferences and constraints for language, architecture, deployment,
  security, governance, budget, runtime, and integration posture.

## Editable Artifacts

This command owns:

- `docs/architecture/system.md`
- `docs/architecture/technology.md`
- `docs/architecture/delivery.md`
- `docs/architecture/operations.md`
- `docs/architecture/subsystems/*.md`

## Responsibilities

1. Read product intent, behavior canon, existing architecture docs, and current
   implementation shape before asking broad questions.
2. Identify architecture decisions already implied by the product and behavior
   catalog.
3. Ask targeted questions for decisions that materially affect roadmap shape:
   service boundaries, storage, runtime, deployment, interfaces, security,
   operations, testing posture, and integration strategy.
4. Separate accepted decisions, open questions, constraints, and rejected
   alternatives.
5. Keep product/user intent out of architecture prose except as rationale.
6. Keep roadmap sequencing out of architecture prose except where a technical
   dependency must constrain roadmap shape.
7. Update high-level and subsystem architecture projections after user
   approval.
8. Summarize architecture decisions that `craft-roadmap` must consume.

## Out of Scope

- Editing product vision, personas, or concepts. Use `plan-product`.
- Editing behavior files. Use `identify-behaviors`.
- Assessing whether current code already implements behaviors. Use
  `assess-implementation`.
- Creating roadmap DAG nodes. Use `craft-roadmap`.
- Writing implementation code.
- Dispatching specs, creating tasks, opening pull requests, or mutating
  orchestration state.
