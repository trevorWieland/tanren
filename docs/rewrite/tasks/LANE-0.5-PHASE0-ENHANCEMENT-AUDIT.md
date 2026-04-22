# Lane 0.5 — Phase 0 Enhancement Audit Checklist

Audit the post-Lane Phase 0 enhancements for secure operation, installation
reliability, generated-artifact completeness/readability, and end-to-end flow
proof reproducibility.

Companion implementation brief:
[LANE-0.5-PHASE0-ENHANCEMENT-BRIEF.md](LANE-0.5-PHASE0-ENHANCEMENT-BRIEF.md)

## Hard Fail Conditions

1. Any insecure MCP startup bypass path exists for local/dev use.
2. Any spec artifact requires manual structured edits for normal operation.
3. Runtime remains dependent on `target/debug/*` assumptions in acceptance path.
4. End-to-end proof cannot be reproduced by a second engineer.

## Audit Checklist

### 1. Runtime Installation and Binary Discoverability

- [ ] Verify documented install path yields callable Tanren binaries on `PATH`.
- [ ] Verify no acceptance step depends on direct `target/debug/*` binary paths.
- [ ] Verify version/health check exists and is machine-checkable.
- [ ] Verify docs no longer conflict between legacy bootstrap and rewrite install.

### 2. Secure MCP Startup (No Opt-Out)

- [ ] Verify MCP startup requires signed capability envelope material.
- [ ] Verify missing envelope envs fail closed with explicit diagnostics.
- [ ] Verify invalid signature/claim scenarios fail closed.
- [ ] Verify installed config/runtime path supports secure startup in practice.

### 3. Command/Transport Clarity for Agents

- [ ] Verify rendered command artifacts contain explicit actionable MCP/CLI
      instructions, not literal binding placeholders.
- [ ] Verify CLI fallback docs/examples match implemented command shape.
- [ ] Verify per-phase invocation examples include required globals and payload
      requirements.
- [ ] Verify capability-denied and capability-parse errors provide actionable
      remediation without trial-and-error.

### 4. Generated Artifact Ownership and Completeness

- [ ] Verify all spec artifacts are generated from typed events/tool calls.
- [ ] Verify no required artifact depends on hand-authored structured state.
- [ ] Verify projection updates all required artifacts after mutating events.
- [ ] Verify ownership/enforcement logic prevents unauthorized direct edits.

### 5. Human Readability Quality Gates

- [ ] Verify generated markdown artifacts are complete and legible.
- [ ] Verify stable section ordering and minimal redundancy.
- [ ] Verify content accuracy against source typed events/state.
- [ ] Verify readability tests/snapshots exist and are meaningful.

### 6. End-to-End Flow Proof (Phase 0)

- [ ] Verify a dedicated flow exercise spec exists and is runnable.
- [ ] Verify orchestration script behavior matches Phase 0 policy:
      direct harness invocation allowed, structured state only via Tanren tools.
- [ ] Verify orchestration entrypoint is non-Python and calls `tanren`
      CLI for status and phase transitions.
- [ ] Verify orchestration resume behavior is store-truth driven via
      `tanren methodology spec status` (no correctness dependency on local
      ad-hoc checkpoints).
- [ ] Verify manual breakouts are explicit and correct:
      missing spec -> `shape-spec`, blocker halt -> `resolve-blockers`,
      walk-ready -> `walk-spec`.
- [ ] Verify Phase 0 harness scope is Codex-only and does not rely on
      Phase 1 runtime/harness adapter abstractions.
- [ ] Verify config-driven hook usage (`task/spec/per-phase`) matches
      `tanren.yml` resolution and is evidenced in run logs.
- [ ] Verify positive + falsification witnesses are present for critical edges.
- [ ] Verify reproduction by another engineer from documented runbook steps.

### 7. Phase 1 Handoff Constraints Captured

- [ ] Verify docs specify shared authoritative DB requirement for orchestrator
      and agent-side Tanren surfaces.
- [ ] Verify credential compatibility requirements are defined for isolated
      runtime topologies (remote VM, containers, no shared filesystem).
- [ ] Verify invariants are testable and explicitly tied to Phase 1 acceptance.

## Required Evidence Artifacts

1. Install/startup transcript from a clean environment.
2. MCP fail-closed negative test outputs.
3. Rendered command sample set showing actionable invocation instructions.
4. Artifact projection sample set covering all required spec artifacts.
5. End-to-end flow exercise logs and proof-pack outputs.
6. Documentation references for Phase 1 shared DB/credential constraints.

## Verdict Rule

Approve only when every checklist item passes and no hard-fail condition is
triggered.
