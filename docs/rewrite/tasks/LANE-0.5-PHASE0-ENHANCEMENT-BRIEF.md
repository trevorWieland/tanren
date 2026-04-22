# Lane 0.5 — Phase 0 Enhancement Brief (Post-Lane Hardening)

## Purpose

Close Phase 0 operator-readiness gaps so agents can execute methodology phases
without trial-and-error, with secure MCP-by-default operation, deterministic
artifact generation, and reproducible end-to-end flow evidence.

This brief is a hardening extension to the merged Lane 0.5 work. It does not
replace the original lane brief; it tightens completion criteria.

## Non-Negotiable Decisions (Locked)

1. No insecure local-dev bypass for MCP capability enforcement. Security model
   must hold in all environments.
2. All spec artifacts are generated and updated via typed, validated Tanren
   tools only. No hand-edited structured artifacts.
3. `tanren` runtime installation is real installation, not a repo-local debug
   assumption. Environments running orchestrator/agents must have callable
   Tanren binaries.
4. Phase 0 demo orchestration may invoke coding harness CLIs directly, but
   structured methodology state changes must still occur through Tanren tools.
5. Human interaction points remain exactly: `shape-spec`, `walk-spec`,
   `resolve-blockers`.
6. Phase 0 orchestration entrypoint must be non-Python (`tanren` CLI-driven
   shell or Rust binary) and must be resumable from store truth.

## Scope

In scope:

1. Runtime install/distribution and secure MCP startup ergonomics.
2. Command phrasing and CLI/MCP invocation clarity.
3. Deterministic generation/readability guarantees for all spec artifacts.
4. End-to-end proof spec and runnable demo orchestration flow for Phase 0.
5. Phase 1 handoff constraints for shared DB/credentialing across isolated
   runtimes.

Out of scope:

1. Planner-native graph scheduling/replanning (Phase 2).
2. Replacing sequential orchestration policy with graph policy.
3. New harness adapter internals (Phase 1 lanes own adapter implementation).

## Read First

1. [LANE-0.5-BRIEF.md](LANE-0.5-BRIEF.md)
2. [LANE-0.5-AUDIT.md](LANE-0.5-AUDIT.md)
3. [../METHODOLOGY_BOUNDARY.md](../METHODOLOGY_BOUNDARY.md)
4. [../../architecture/orchestration-flow.md](../../architecture/orchestration-flow.md)
5. [../../architecture/agent-tool-surface.md](../../architecture/agent-tool-surface.md)
6. [../../architecture/evidence-schemas.md](../../architecture/evidence-schemas.md)
7. [../../architecture/install-targets.md](../../architecture/install-targets.md)
8. [../PHASE0_PROOF_BDD.md](../PHASE0_PROOF_BDD.md)
9. [../PHASE0_PROOF_RUNBOOK.md](../PHASE0_PROOF_RUNBOOK.md)
10. [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md)
11. [../ROADMAP.md](../ROADMAP.md)

## Implementor Checklist

### A. Runtime Install Is Real and Reproducible

- [ ] Define supported installation path for `tanren-cli` and `tanren-mcp`
      suitable for clean machines (not `target/debug/*` assumptions).
- [ ] Ensure install flow yields callable commands on `PATH` in target
      environments used by orchestrator and agents.
- [ ] Add machine-checkable runtime verification command/doc step that proves:
      binary presence, expected version, and executable status.
- [ ] Update docs to remove ambiguity between legacy bootstrap/install guidance
      and rewrite methodology install flow.

### B. Secure MCP Startup With No Bypass

- [ ] Keep signed capability-envelope verification mandatory at MCP startup.
- [ ] Ensure installer/config path provides everything required for MCP startup
      in real runs (issuer/audience/key/envelope plumbing via orchestrator).
- [ ] Document canonical secure startup sequence and failure diagnostics.
- [ ] Add tests that prove startup fails closed when required envelope material
      is missing/invalid.

### C. Agent Invocation Clarity (No Trial-and-Error)

- [ ] Replace `{{TASK_TOOL_BINDING}}` literal rendering (`mcp`/`cli`) with
      explicit actionable instructions per target binding.
- [ ] Align CLI fallback documentation with implemented syntax and payload
      shapes (`tanren methodology ... --json ...`).
- [ ] Provide canonical per-phase invocation examples with required globals
      (`--phase`, `--spec-id`, `--spec-folder`) and valid payload fields.
- [ ] Improve capability-denied and scope-parse errors to include:
      attempted capability, active phase, granted capabilities, and concrete
      remediation.

### D. All Spec Artifacts Are Tool-Generated and Human-Readable

- [ ] Make every spec artifact generated from typed events/tool calls,
      including audit and signpost artifacts.
- [ ] Enforce generated-artifact ownership and update guarantees for:
      `spec.md`, `plan.md`, `tasks.md`, `tasks.json`, `demo.md`,
      `progress.json`, `audit.md`, `signposts.md`, `phase-events.jsonl`,
      and manifest artifacts.
- [ ] Define and enforce readability standards for generated markdown:
      complete, legible, minimally redundant, stable ordering, accurate.
- [ ] Add snapshot/integration tests to guard readability and structural
      completeness of generated markdown.

### E. Phase 0 End-to-End Demo Spec and Proof

- [ ] Add a dedicated Phase 0 flow exercise spec that runs the canonical
      sequential loop and captures evidence.
- [ ] Implement a deterministic demo orchestration script for Phase 0 that
      can call harness CLIs directly but records structured state only through
      Tanren tool calls.
- [ ] Orchestration script entrypoint is non-Python and invokes `tanren`
      CLI contracts for status/gates/phase state transitions.
- [ ] Orchestration script is auto-resumable from store truth
      (`tanren methodology spec status`) and never depends on ad-hoc local
      state files for correctness.
- [ ] Missing spec path halts with explicit `shape-spec` prompt; blocker path
      halts with explicit `resolve-blockers` prompt; walk-ready path halts
      with explicit `walk-spec` prompt.
- [ ] Harness scope in Phase 0 is Codex-only for autonomous steps; no Phase 1
      runtime/harness abstraction is introduced here.
- [ ] Gate hook execution is config-driven (`tanren.yml` resolution for
      `task_verification_hook`, `spec_verification_hook`, per-phase overrides)
      and directly verified in run artifacts.
- [ ] Include positive and falsification witnesses for key flow edges:
      capability denial, invalid payload, guard convergence, escalation path.
- [ ] Produce reproducible runbook steps and proof-pack outputs so a second
      engineer can reproduce results.

### F. Phase 1 Handoff Constraints (Must Be Captured Now)

- [ ] Document explicit contract that orchestrator Tanren and agent-side Tanren
      (MCP/CLI) must use the same authoritative database and compatible
      credential model.
- [ ] Document constraints for isolated runtime topologies (remote VM,
      containerized worker, no shared filesystem, no shared local process).
- [ ] Define required cross-runtime invariants for Phase 1 acceptance:
      same DB truth, typed event parity, credential compatibility.

## Deliverables

1. Updated architecture/docs for secure MCP startup, real install path,
   and transport invocation clarity.
2. Installer/runtime updates implementing real install and secure startup
   requirements.
3. Tool/error-message updates that remove phase-capability guesswork.
4. Artifact projection updates covering all spec artifacts with readability
   quality gates.
5. New/updated Phase 0 proof assets and end-to-end flow exercise script/spec.
6. Explicit Phase 1 handoff note covering shared DB/credentialing across
   isolated runtimes.

## Done When

1. Fresh environment can install Tanren runtime and execute required commands
   without repo-local debug paths.
2. MCP starts securely with required signed-envelope controls and fails closed
   on missing/invalid claims.
3. Generated command artifacts tell agents exactly how to execute phase actions
   without trial-and-error.
4. All spec artifacts are generated, current, and human-readable under
   deterministic projection rules.
5. Phase 0 end-to-end flow exercise passes with reproducible evidence pack.
6. Phase 1 handoff constraints for shared DB/credential compatibility across
   isolated runtimes are documented and testable.

## Auditor Handoff Packet Required From Implementor

1. Command transcript proving install/startup on clean environment.
2. MCP startup negative tests (fail-closed evidence).
3. Before/after examples of rendered command instructions.
4. Artifact projection proof set for all spec artifacts.
5. End-to-end exercise run output and falsification witnesses.
6. Phase 1 handoff invariants document reference.

## Follow-On Alignment Notes

1. Phase 1 must end with orchestration scripts interacting through Tanren
   commands/APIs only, not direct harness CLI calls.
2. Phase 2 must replace temporary sequential shell orchestration with a user-
   facing autonomous flow entry point while preserving the three interactive
   checkpoints (`shape-spec`, `walk-spec`, `resolve-blockers`).
