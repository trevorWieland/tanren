---
name: design-experience
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
  - docs/experience/flows.md
  - docs/experience/screens.md
  - docs/experience/interaction-models.md
  - docs/experience/state-matrix.md
  - docs/experience/proof-matrix.md
---

# design-experience

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes experience
projections directly because native experience-contract schemas, typed tools,
and project-method events do not exist yet. Prefer behavior-linked records,
surface-specific proof obligations, and compact reviewable edits so these
artifacts can later migrate into typed Tanren storage.

This command is for any repository adopting the Tanren method. Use the
repository's configured experience artifact paths; if none are configured, use
the conventional `docs/experience/` path.

## Purpose

Turn accepted behavior and project surfaces into concrete experience contracts:
entry points, flows, states, copy obligations, interaction rules, and proof
artifacts for each behavior-surface pair.

This command keeps Tanren from treating UI/UX as web-only. A terminal command,
TUI screen, game replay, SDK example, API contract, chat transcript, and mobile
view can all be valid experience contracts when they are the surface where the
behavior is actually observed.

## Inputs

- Product projections from `docs/product/**`.
- Accepted behavior catalog from `docs/behaviors/**`.
- Surface registry from `docs/experience/surfaces.yml`.
- Architecture projections from `docs/architecture/**`.
- Existing UI, command, game, SDK, API, or agent interaction patterns.
- Human feedback, usability findings, support examples, and review notes.

## Editable Artifacts

This command owns:

- `docs/experience/flows.md`
- `docs/experience/screens.md`
- `docs/experience/interaction-models.md`
- `docs/experience/state-matrix.md`
- `docs/experience/proof-matrix.md`

## Responsibilities

1. Read the behavior catalog and active surface registry before proposing
   experience work.
2. For each relevant behavior-surface pair, identify entry point, primary flow,
   success state, failure states, empty/loading/stale/unavailable states, and
   recovery paths.
3. Define surface-native proof obligations: screenshots for GUI, transcripts
   for CLI/TUI/chat, deterministic replay for games, contract examples for
   APIs and libraries.
4. Record accessibility, localization, latency, copy, and feedback expectations
   at the surface level.
5. Keep implementation choices out unless the architecture has already accepted
   them.
6. Mark unknown proof adapters or high-risk interactions so `craft-roadmap` can
   size and sequence the work honestly.
7. Summarize changed experience contracts, unresolved decisions, UX risks, and
   proof gaps.

## Out of Scope

- Defining project surfaces. Use `define-surfaces`.
- Editing product vision, personas, or concepts. Use `plan-product`.
- Adding or removing accepted behaviors. Use `identify-behaviors`.
- Choosing implementation architecture. Use `architect-system`.
- Creating roadmap DAG nodes. Use `craft-roadmap`.
