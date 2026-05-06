# Gameplay Experience Contracts

Use this profile when a behavior is primarily experienced through an
interactive game loop, level, scene, or simulation.

## Standards

- Every behavior-surface contract names the playable state, input map, feedback
  timing, success condition, failure condition, pause or interruption behavior,
  and persistence expectations.
- Core mechanics must be proven through deterministic input replay or an
  engine-native test harness rather than visual inspection alone.
- Save/load, retry, reset, and progression rules must be explicit whenever a
  behavior can change durable player state.
- Accessibility options relevant to the mechanic, such as remapping, captions,
  color independence, timing tolerance, or reduced motion, must be listed.
- Frame-time, input-latency, and simulation determinism expectations must be
  stated for critical real-time behavior.

## Proof Adapter

- Replay deterministic input against the real gameplay surface or engine test
  harness.
- Assert scene state, durable progression, and failure recovery.
- Capture screenshot, clip, or telemetry evidence for human walks.
- Include at least one falsification replay for invalid, blocked, or failed
  player action when meaningful.
