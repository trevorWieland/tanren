---
schema: tanren.subsystem_architecture.v0
subsystem: design-system
status: draft
owner_command: architect-system
updated_at: 2026-05-07
---

# Design System Architecture

## Purpose

Tanren needs a central design canon so human and agent-generated work does not
drift into unrelated local UI, command, terminal, contract, or gameplay
patterns.

The design system is cross-surface. It is not only a React component library.
It defines reusable experience decisions that can be projected into web,
terminal, API, MCP, game, mobile, desktop, library, chat, and other project
surfaces.

## Target Model

Each project maintains a design-system projection under:

```text
docs/experience/design-system/
  README.md
  principles.md
  tokens.yml
  vocabulary.yml
  patterns.yml
  accessibility.md
  surface-adapters/
    web.md
    cli.md
    tui.md
    api.md
    mcp.md
    gameplay.md
```

`define-design-system` owns these records. `design-experience` consumes them
when authoring behavior-surface contracts. `craft-roadmap` cites pattern IDs
through `design_pattern_refs` so shaped work can carry design obligations
before implementation begins.

## Records

Design-system records are project-local:

- **principles**: stable decision rules that guide all surfaces;
- **tokens**: semantic roles such as status, intent, focus, density, and
  content emphasis;
- **vocabulary**: shared product terms, state names, error codes, and copy
  expectations;
- **patterns**: reusable flow, feedback, output, navigation, confirmation,
  layout, and proof decisions;
- **accessibility**: cross-surface accessibility requirements;
- **surface adapters**: how shared records project into each surface kind.

Pattern IDs use lowercase dot-separated namespaces:

```text
feedback.validation.inline
action.destructive.confirmation
output.machine.error-envelope
layout.tui.status-bar
gameplay.retry.preserve-progress
```

They name reusable experience decisions, not components, routes, functions,
screens, commands, or engine objects.

## Method Chain

The planning chain becomes:

```text
plan-product
-> define-surfaces
-> define-design-system
-> identify-behaviors
-> design-experience
-> architect-system
-> assess-implementation
-> craft-roadmap
-> shape-spec / orchestrate / prove / walk
```

`define-design-system` belongs after `define-surfaces` because the design
system needs to know which surfaces exist. It belongs before
`design-experience` because behavior-surface contracts should reference
registered tokens, vocabulary, and pattern IDs instead of inventing local
rules.

## Drift Model

Design drift happens when generated work introduces a local experience decision
that is not traceable to accepted design-system records. Examples:

- a web form uses local colors, spacing, or validation behavior;
- a CLI command invents a new exit-code meaning;
- a TUI screen adds a new key binding that conflicts with the keyboard model;
- an API returns an unregistered error code;
- a game retry loop changes durable progress without a registered pattern;
- a roadmap node claims experience work without citing the patterns it must
  honor.

Tanren should treat those as planning or proof defects. During bootstrap,
`scripts/roadmap_check.py` validates roadmap `design_pattern_refs` against
`docs/experience/design-system/patterns.yml`. Later typed Tanren state should
validate design-system references on experience contracts, specs, proof
artifacts, and walk records.

## Generation Workflow

When agents generate user-facing or client-facing work, they should:

1. Read product intent and accepted behavior.
2. Read the surface registry.
3. Read the design-system records and selected pattern IDs.
4. Read behavior-surface experience contracts.
5. Generate implementation and proof artifacts using the surface adapter.
6. Produce review evidence that names behavior ID, surface ID, and pattern ID.
7. Record explicit deviations for human review when no registered pattern fits.

## Acceptance Criteria

This model is complete when:

- every project can define reusable cross-surface design tokens, vocabulary,
  patterns, and adapters;
- behavior-surface contracts can reference registered design patterns;
- roadmap nodes can cite `design_pattern_refs`;
- validators reject unknown design pattern references;
- web-specific component systems are profile-specific projections of the
  design system, not the whole design system;
- walk evidence can be traced to behavior, surface, and pattern IDs.
