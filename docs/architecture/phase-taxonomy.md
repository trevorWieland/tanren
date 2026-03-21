# Phase Taxonomy

Orchestration phases have properties on multiple orthogonal axes. Understanding
these axes clarifies which configuration knobs apply where — and surfaces gaps
where a new phase or configuration option might be needed.

## Axis: Execution Mode

How the phase runs.

| Mode | Description | Phases |
|---|---|---|
| **AGENTIC** | Prompt + CLI harness (opencode, codex, claude) | do-task, audit-task, run-demo, audit-spec, investigate |
| **AUTOMATED** | Raw shell command, pass/fail | gate, setup, cleanup |

## Axis: Intent

What the phase does to the codebase.

| Intent | Description | Phases |
|---|---|---|
| **CHANGING** | Modifies source code | do-task |
| **CHECKING** | Validates prior work without modifying code | gate, audit-task, run-demo, audit-spec |
| **TRIAGING** | Interprets a failed CHECK to diagnose root cause | investigate |
| **INFRA** | Manages execution environment, not the codebase | setup, cleanup |

## Axis: Scope

What slice of the work the phase operates on.

| Scope | Description | Phases |
|---|---|---|
| **TASK** | Single task slice | do-task, gate, audit-task |
| **SPEC** | Whole-spec completeness | run-demo, audit-spec |
| **CONTEXT-DEPENDENT** | Could serve either scope | investigate |
| **INFRA** | Not scoped to tasks or spec | setup, cleanup |

## Combined View

| Phase | Execution Mode | Intent | Scope |
|---|---|---|---|
| do-task | AGENTIC | CHANGING | TASK |
| gate | AUTOMATED | CHECKING | TASK |
| audit-task | AGENTIC | CHECKING | TASK |
| run-demo | AGENTIC | CHECKING | SPEC |
| audit-spec | AGENTIC | CHECKING | SPEC |
| investigate | AGENTIC | TRIAGING | CONTEXT-DEPENDENT |
| setup | AUTOMATED | INFRA | INFRA |
| cleanup | AUTOMATED | INFRA | INFRA |

## Gate Command Resolution

The **Scope** axis determines which gate command is used. Fast task-level gates
(~2 min: lint, type-check, unit tests) run after task work. Thorough spec-level
gates (~15 min: integration, e2e) run after spec-level validation.

### tanren.yml Configuration

```yaml
environment:
  default:
    gate_cmd: make check          # fallback for all phases
    task_gate_cmd: make unit      # used for task-scoped gates
    spec_gate_cmd: make e2e       # used for spec-scoped gates
```

All three fields are optional. `gate_cmd` defaults to `make check`.
`task_gate_cmd` and `spec_gate_cmd` default to `null` (fall back to `gate_cmd`).

### Resolution Rules

The triggering phase (the logical phase whose work is being gated, not
necessarily the dispatch's own phase) determines which command is resolved:

| Triggering Phase | Resolution |
|---|---|
| do-task, gate, audit-task | `task_gate_cmd` if set, else `gate_cmd` |
| run-demo, audit-spec | `spec_gate_cmd` if set, else `gate_cmd` |
| setup, cleanup, investigate | `gate_cmd` |

Note: both `task_gate` and `spec_gate` are dispatched as `phase=gate`. The
coordinator knows the gate scope from its state machine sub-state and passes
the appropriate triggering phase (e.g. `Phase.DO_TASK` for task gates,
`Phase.RUN_DEMO` for spec gates).

### Priority Chain

Highest priority wins:

1. CLI `--gate-cmd` flag (explicit user override)
2. `GateExpectation.gate_command_override` (per-task override from shape-spec)
3. Phase-specific field (`task_gate_cmd` or `spec_gate_cmd`)
4. `gate_cmd` (profile default)

Implementation:
`tanren_core.env.gates.resolve_gate_cmd(profile, triggering_phase)`
handles steps 3-4. Steps 1-2 are caller responsibilities.

## Open Questions

- Is there a meaningful prompt difference between spec-scope and task-scope
  INVESTIGATE, or is it fundamentally the same problem?
- Should auto-formatters and auto-fixers be a distinct AUTOMATED + CHANGING
  phase, or remain embedded in gate commands?

## Related Docs

- [Spec Lifecycle](../workflow/spec-lifecycle.md) — orchestration loop
- [Protocol README](../../protocol/README.md) — Protocol overview
