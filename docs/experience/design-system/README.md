---
schema: tanren.design_system_index.v0
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# Design System

This directory defines Tanren's cross-surface design system. It is the central
experience canon agents should read before generating UI, command output, TUI
screens, machine contracts, gameplay flows, or other project surfaces.

The design system is broader than a web component library:

- `principles.md` defines shared experience principles.
- `tokens.yml` defines semantic tokens that can be projected into surfaces.
- `vocabulary.yml` defines shared product, state, and error language.
- `patterns.yml` defines stable reusable pattern IDs.
- `accessibility.md` defines cross-surface accessibility expectations.
- `surface-adapters/` explains how the shared system appears on each surface
  kind.

## Ownership

`define-design-system` owns these files. `design-experience` consumes them when
authoring behavior-surface contracts. `craft-roadmap` may cite pattern IDs in
roadmap nodes through `design_pattern_refs` so implementation agents know which
central patterns the work must follow.

## Drift Rule

Generated work should reference registered design tokens, vocabulary, and
pattern IDs. A new local visual, output, copy, or interaction pattern should be
added here first or recorded as an explicit deviation in the experience
contract.
