---
schema: tanren.design_principles.v0
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# Design Principles

## Behavior First

Every designed surface exists to make accepted behavior observable. Visual
style, command grammar, terminal layout, contract shape, and game feedback are
secondary to the behavior the user or client must complete.

## Surface Native

Each surface should feel native to its medium. Web work can use routes,
forms, responsive layout, and screenshots. CLI work should use command grammar,
stdout, stderr, exit codes, and transcripts. TUI work should use focus,
keyboard navigation, status bars, and PTY snapshots. Games should use input
maps, feedback timing, replays, and scene state.

## One Vocabulary

The same product state should use the same words and machine codes across
surfaces. A `validation_failed` response, a CLI validation message, a TUI
inline error, and a web form error are different presentations of the same
concept.

## Tokens Before One-Offs

Colors, spacing, status intent, motion, density, and copy tone should be
described by semantic tokens before a surface adapter maps them to CSS,
terminal attributes, response fields, or game UI settings.

## Proofable States

Every pattern must be reviewable through a surface-native proof artifact:
screenshot, transcript, PTY capture, deterministic replay, contract case,
schema example, or tool-call trace.

## Accessibility Is A Contract

Accessibility is not web-only. Keyboard operation, readable text, color
independence, reduced motion, machine-readable errors, screen-reader-friendly
output, and configurable timing are design-system concerns.

## Drift Is A Defect

Agents should not invent local experience rules when a registered token,
vocabulary term, or pattern ID applies. If a new pattern is needed, add it to
the design system or record an explicit deviation for review.
