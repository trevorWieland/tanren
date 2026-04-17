# Phase Taxonomy

Orchestration phases have properties on multiple orthogonal axes.
Understanding these axes clarifies which configuration knobs apply
where — and surfaces gaps where a new phase or configuration option
might be needed.

## Axis: Execution Mode

How the phase runs.

| Mode | Description | Phases |
|---|---|---|
| **AGENTIC** | Prompt + CLI harness (Claude Code, Codex, OpenCode) | shape-spec, do-task, audit-task, adhere-task, run-demo, audit-spec, adhere-spec, walk-spec, handle-feedback, investigate, resolve-blockers |
| **AUTOMATED** | Raw shell command, pass/fail | task-gate, spec-gate, setup, cleanup |

## Axis: Intent

What the phase does to the codebase.

| Intent | Description | Phases |
|---|---|---|
| **SHAPING** | Collaborative scope definition with a human | shape-spec |
| **CHANGING** | Modifies source code | do-task |
| **CHECKING** (rubric) | Opinionated scored judgment; produces findings | audit-task, audit-spec |
| **CHECKING** (adherence) | Standards-based pass/fail compliance | adhere-task, adhere-spec |
| **CHECKING** (automated) | Shell gate invocation | task-gate, spec-gate, run-demo |
| **VALIDATING** | Human-facing acceptance walkthrough | walk-spec |
| **FEEDBACK** | Post-PR review triage | handle-feedback |
| **TRIAGING** | Autonomous failure diagnosis | investigate |
| **RESOLVING** | Interactive blocker resolution | resolve-blockers |
| **INFRA** | Manages execution environment | setup, cleanup |

## Axis: Scope

What slice of the work the phase operates on.

| Scope | Description | Phases |
|---|---|---|
| **TASK** | Single task slice | do-task, task-gate, audit-task, adhere-task |
| **SPEC** | Whole-spec completeness | shape-spec, run-demo, audit-spec, adhere-spec, walk-spec, spec-gate |
| **CROSS-PHASE** | Operates on feedback across a whole PR / review | handle-feedback |
| **CONTEXT-DEPENDENT** | Serves either task or spec scope depending on dispatch | investigate, resolve-blockers |
| **INFRA** | Not scoped to tasks or spec | setup, cleanup |

## Axis: Autonomy

Whether a human is in the loop during execution.

| Autonomy | Phases |
|---|---|
| **INTERACTIVE** | shape-spec, walk-spec, resolve-blockers |
| **AUTONOMOUS** | every other agentic + automated phase |

Exactly three phases are interactive. Investigate is the autonomous
escalation mechanism; it promotes to a blocker (triggering
resolve-blockers) only after its loop cap is hit.

## Combined View

| Phase | Mode | Intent | Scope | Autonomy | Emits Guard? |
|---|---|---|---|---|---|
| shape-spec | AGENTIC | SHAPING | SPEC | INTERACTIVE | — |
| setup | AUTOMATED | INFRA | INFRA | AUTONOMOUS | — |
| do-task | AGENTIC | CHANGING | TASK | AUTONOMOUS | `TaskImplemented` |
| task-gate | AUTOMATED | CHECKING (auto) | TASK | AUTONOMOUS | `TaskGateChecked` |
| audit-task | AGENTIC | CHECKING (rubric) | TASK | AUTONOMOUS | `TaskAudited` |
| adhere-task | AGENTIC | CHECKING (adherence) | TASK | AUTONOMOUS | `TaskAdherent` |
| spec-gate | AUTOMATED | CHECKING (auto) | SPEC | AUTONOMOUS | — |
| run-demo | AGENTIC | CHECKING (auto) | SPEC | AUTONOMOUS | — |
| audit-spec | AGENTIC | CHECKING (rubric) | SPEC | AUTONOMOUS | — |
| adhere-spec | AGENTIC | CHECKING (adherence) | SPEC | AUTONOMOUS | — |
| walk-spec | AGENTIC | VALIDATING | SPEC | INTERACTIVE | — |
| handle-feedback | AGENTIC | FEEDBACK | CROSS-PHASE | AUTONOMOUS | — |
| investigate | AGENTIC | TRIAGING | CONTEXT-DEPENDENT | AUTONOMOUS | — |
| resolve-blockers | AGENTIC | RESOLVING | CONTEXT-DEPENDENT | INTERACTIVE | — |
| cleanup | AUTOMATED | INFRA | INFRA | AUTONOMOUS | — |

## Guards and Task Completion

Task completion uses a multi-guard model: `Implemented` task
transitions to `Complete` when all required forward guards are
satisfied. Default required set (config-defined):

- `gate_checked` — emitted by task-gate on pass
- `audited` — emitted by audit-task on pass (zero `fix_now` findings)
- `adherent` — emitted by adhere-task on pass (zero `fix_now`
  adherence findings)

Guards run independently and can execute in parallel. See
[orchestration-flow.md](orchestration-flow.md) §2 for the full state
machine. Required guards are configured in `tanren.yml`:

```yaml
methodology:
  task_complete_requires: [gate_checked, audited, adherent]
```

Adding a new guard (e.g. `security_reviewed`, `load_tested`) is a
two-line change: new event variant + config entry. Existing code
paths are unchanged.

## Verification Hook Resolution

The workflow layer resolves verification hooks by **command/phase
key**. Shared command markdown refers only to template variables
(`{{TASK_VERIFICATION_HOOK}}`, `{{SPEC_VERIFICATION_HOOK}}`,
phase-specific overrides) that the installer renders at install time;
commands never resolve hooks themselves.

### tanren.yml Configuration

```yaml
environment:
  default:
    gate_cmd: just check             # fallback for all phases
    task_gate_cmd: just check        # task-scoped gates
    spec_gate_cmd: just ci           # spec-scoped gates
    verification_hooks:
      do-task: just check
      audit-task: just check
      adhere-task: just check
      run-demo: just ci
      audit-spec: just ci
      adhere-spec: just ci
```

`verification_hooks` is the preferred shape because it keys directly
by the logical phase whose work is being verified. Legacy
`task_gate_cmd` / `spec_gate_cmd` remain compatibility shims.

### Resolution Rules

The triggering phase (logical phase whose work is being verified)
determines which command is resolved. Priority order:

1. CLI `--gate-cmd` flag (explicit user override)
2. `GateExpectation.gate_command_override` (per-task override from
   shape-spec)
3. Phase-keyed verification hook
   (`verification_hooks.<triggering_phase>`)
4. Legacy scoped field (`task_gate_cmd` / `spec_gate_cmd`)
5. `verification_hooks.default`
6. `gate_cmd`

**Tanren code is responsible for this resolution.** Shared command
markdown never embeds a literal verification command; it references
the appropriate template variable.

### Command/Phase Key Registry

Recognized keys under `verification_hooks`:

- `do-task`, `audit-task`, `adhere-task` (task-scoped)
- `run-demo`, `audit-spec`, `adhere-spec` (spec-scoped)
- `default` (catch-all)

Unknown keys emit a config warning; unknown phases fall back through
the priority chain.

## Related Docs

- [Orchestration flow](orchestration-flow.md) — full state machine
- [Agent tool surface](agent-tool-surface.md) — how commands talk to
  the orchestrator
- [Evidence schemas](evidence-schemas.md) — per-phase output
  documents
- [Audit rubric](audit-rubric.md) — pillar semantics for rubric
  phases
- [Adherence](adherence.md) — standards-check semantics
- [Install targets](install-targets.md) — rendered command outputs
- [Design rationale](../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md)
