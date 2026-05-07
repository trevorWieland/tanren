---
schema: tanren.design_surface_adapter.v0
surface_kind: terminal_ui
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# TUI Adapter

The TUI adapter projects shared design-system records into terminal layout,
focus, keyboard navigation, status bars, and screen snapshots.

## Projection

- Surface and content tokens map to terminal attributes and symbols.
- Intent tokens must be readable in monochrome.
- Pattern IDs map to reusable screen regions, key paths, status messages, and
  recovery flows.
- `layout.tui.status-bar` is the default persistent status pattern for
  operator-facing screens.

## Required Evidence

- PTY-driven keyboard-path proof.
- Screen snapshots at fixed terminal sizes.
- Resize proof for screens with multiple regions.
- Golden transcripts where text output is the review artifact.

## Drift Signals

- Mouse-only paths for required behavior.
- Status represented by color only.
- New key bindings that conflict with the surface interaction model.
