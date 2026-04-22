---
schema_version: v1
kind: demo
spec_id: 00000000-0000-0000-0000-000000000c01
environment:
  probed_at: 2026-04-22T14:14:36.909690Z
  connections_verified: true
steps:
- id: demo-01
  mode: RUN
  description: Run behavior scenario gate and show behavior-ID-tagged scenario pass output.
  expected_observable: Scenario runner output shows passing BEH-* tagged scenarios.
- id: demo-02
  mode: RUN
  description: Run mutation gate and show pass/survivor triage evidence.
  expected_observable: Mutation output passes or records survivors with behavior-linked triage.
- id: demo-03
  mode: RUN
  description: Run coverage gate and show uncovered-path classification artifact.
  expected_observable: Coverage artifact classifies missing-behavior vs dead/support code.
- id: demo-04
  mode: RUN
  description: Run just check at task-stage and confirm green after staged changes.
  expected_observable: just check exits successfully with staged framework active.
- id: demo-05
  mode: RUN
  description: Run final just ci and verify full strict gate closure.
  expected_observable: just ci exits successfully including behavior, mutation, and coverage enforcement.
- id: demo-06
  mode: SKIP
  description: Execute remote matrix verification step for workflow parity.
  expected_observable: GitHub Actions matrix run confirms workflow parity.
  skip_reason: Requires remote GitHub Actions execution; not runnable in local shaping environment.
results: []
---
# Demo

## Steps
- demo-01 [RUN] Run behavior scenario gate and show behavior-ID-tagged scenario pass output.
- demo-02 [RUN] Run mutation gate and show pass/survivor triage evidence.
- demo-03 [RUN] Run coverage gate and show uncovered-path classification artifact.
- demo-04 [RUN] Run just check at task-stage and confirm green after staged changes.
- demo-05 [RUN] Run final just ci and verify full strict gate closure.
- demo-06 [SKIP] Execute remote matrix verification step for workflow parity.
