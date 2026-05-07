---
name: define-design-system
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
  - docs/experience/design-system/README.md
  - docs/experience/design-system/principles.md
  - docs/experience/design-system/tokens.yml
  - docs/experience/design-system/vocabulary.yml
  - docs/experience/design-system/patterns.yml
  - docs/experience/design-system/accessibility.md
  - docs/experience/design-system/surface-adapters/*.md
---

# define-design-system

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes design-system
projections directly because native design-system schemas, typed tools, and
project-method events do not exist yet. Prefer stable token IDs, stable pattern
IDs, explicit surface adapters, and compact reviewable edits so these artifacts
can later migrate into typed Tanren storage.

This command is for any repository adopting the Tanren method. Use the
repository's configured design-system artifact paths; if none are configured,
use the conventional `docs/experience/design-system/` path.

## Purpose

Define the cross-surface design system that keeps generated experience coherent
as humans and agents implement work across web, terminal, API, MCP, game,
mobile, desktop, library, chat, and other project surfaces.

A Tanren design system is not only a web component library. It is the shared
experience canon for tokens, vocabulary, interaction patterns, accessibility
requirements, evidence expectations, and surface-specific adapters.

## Inputs

- Product projections from `docs/product/**`.
- Surface registry from `docs/experience/surfaces.yml`.
- Existing experience projections from `docs/experience/**`.
- Existing UI components, command output, terminal screens, API contracts,
  game loops, SDK examples, chat transcripts, or other surface artifacts.
- Human preferences and constraints for brand, tone, density, accessibility,
  localization, latency, evidence, and supported devices.

## Editable Artifacts

This command owns:

- `docs/experience/design-system/README.md`
- `docs/experience/design-system/principles.md`
- `docs/experience/design-system/tokens.yml`
- `docs/experience/design-system/vocabulary.yml`
- `docs/experience/design-system/patterns.yml`
- `docs/experience/design-system/accessibility.md`
- `docs/experience/design-system/surface-adapters/*.md`

## Temporary Artifact Format

`patterns.yml` declares the stable pattern IDs that roadmap nodes and
experience contracts may reference:

```yaml
schema: tanren.design_patterns.v0
updated_at: YYYY-MM-DD
owner_command: define-design-system
patterns:
  - id: feedback.validation.inline
    name: Inline validation feedback
    surfaces: [web, cli, tui, api, mcp]
    intent: Explain invalid input where the user or client can recover.
    required_states: [validation_failed]
    evidence: [screenshot, transcript, contract_case]
```

Pattern IDs use lowercase dot-separated namespaces. They should name reusable
experience decisions, not one-off screens or implementation modules.

## Responsibilities

1. Read product intent, personas, surface registry, existing experience
   projections, and current implementation patterns before proposing tokens or
   pattern IDs.
2. Define cross-surface principles that are specific enough to guide agents
   without becoming surface implementation detail.
3. Define semantic tokens and vocabulary that can be projected into each
   surface.
4. Define reusable design pattern IDs for common flows, states, outputs,
   feedback, navigation, confirmation, and proof evidence.
5. Define surface adapters that explain how the shared system appears on each
   surface kind.
6. Mark unresolved brand, copy, accessibility, localization, or proof decisions
   with explicit `*-pending` tokens.
7. Summarize added, changed, deprecated, and unresolved design-system records,
   especially changes that should raise roadmap `experience_risk`.

## Out of Scope

- Defining project surfaces. Use `define-surfaces`.
- Designing per-behavior flows and states. Use `design-experience`.
- Editing product vision, personas, or concepts. Use `plan-product`.
- Choosing implementation architecture. Use `architect-system`.
- Creating roadmap DAG nodes. Use `craft-roadmap`.
