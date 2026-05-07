---
schema: tanren.design_surface_adapter.v0
surface_kind: interactive_realtime
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# Gameplay Adapter

The gameplay adapter projects shared design-system records into playable state,
input maps, HUD or scene feedback, timing, replay proof, and persistence rules.

## Projection

- Pattern IDs map to mechanics, feedback timing, failure recovery, and durable
  progression rules.
- Intent tokens map to feedback roles; the implementation may realize them as
  sound, text, animation, haptics, HUD state, or scene state.
- Accessibility records must name remapping, captions, reduced motion,
  color independence, or timing tolerance when relevant.

## Required Evidence

- Deterministic replay or engine-native behavior proof.
- Scene state assertions for success and failure.
- Screenshot, clip, or telemetry evidence for human walks.
- Save/load and retry proof when behavior changes durable progress.

## Drift Signals

- Visual-only proof for a mechanic that can be asserted through replay.
- Timing-sensitive behavior without frame-time or input-latency expectations.
- Retry, reset, or save-state behavior not tied to a registered pattern.
