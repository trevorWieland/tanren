---
schema: tanren.experience_index.v0
status: draft
owner_command: design-experience
updated_at: 2026-05-05
---

# Experience Projections

This directory holds project-surface and behavior-surface experience
projections.

- `surfaces.yml` is owned by `define-surfaces` and declares the active public
  surfaces a project can prove behavior through.
- `design-system/` is owned by `define-design-system` and declares the
  cross-surface tokens, vocabulary, patterns, accessibility expectations, and
  surface adapters agents should use when generating experience.
- `state-matrix.md` is owned by `design-experience` and lists the states each
  behavior-surface pair should consider before shaping work.
- `proof-matrix.md` is owned by `design-experience` and maps each surface to
  the proof artifacts that make behavior evidence reviewable.

Future projections may add flows, screens, interaction models, transcripts,
replays, examples, and walk evidence indexes as typed Tanren storage lands.
