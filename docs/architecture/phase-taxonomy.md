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

## Verification Hook Resolution

The workflow layer resolves verification hooks by **command/phase key**, not
by prompt-local convention. Shared markdown should refer only to the resolved
verification hook for the active workflow state.

### tanren.yml Configuration

```yaml
environment:
  default:
    gate_cmd: make check          # fallback for all phases
    task_gate_cmd: make unit      # used for task-scoped gates
    spec_gate_cmd: make e2e       # used for spec-scoped gates
    verification_hooks:
      do-task: just check
      audit-task: just check
      run-demo: just ci
      audit-spec: just ci
```

`verification_hooks` is the preferred shape because it keys directly by the
logical phase whose work is being verified. The older `task_gate_cmd` and
`spec_gate_cmd` fields remain compatibility shims for now.

### Resolution Rules

The triggering phase (the logical phase whose work is being verified, not
necessarily the dispatch's own `gate` dispatch) determines which command is
resolved:

| Priority | Resolution |
|---|---|
| 1 | `verification_hooks[triggering_phase]` |
| 2 | legacy scoped field (`task_gate_cmd` / `spec_gate_cmd`) when applicable |
| 3 | `verification_hooks.default` |
| 4 | `gate_cmd` |

Note: both task-scope and spec-scope verification may still be dispatched as
`phase=gate`; the workflow layer carries the logical triggering phase.

### Priority Chain

Highest priority wins:

1. CLI `--gate-cmd` flag (explicit user override)
2. `GateExpectation.gate_command_override` (per-task override from shape-spec)
3. Phase-keyed verification hook (`verification_hooks.<phase>`)
4. Legacy scoped field (`task_gate_cmd` or `spec_gate_cmd`)
5. `verification_hooks.default`
6. `gate_cmd`

Tanren code is responsible for this resolution. Shared command markdown should
never embed a literal verification command.

## Open Questions

- Is there a meaningful prompt difference between spec-scope and task-scope
  INVESTIGATE, or is it fundamentally the same problem?
- Should auto-formatters and auto-fixers be a distinct AUTOMATED + CHANGING
  phase, or remain embedded in gate commands?

## Related Docs

- [Spec Lifecycle](../workflow/spec-lifecycle.md) — orchestration loop
- [Protocol README](../../protocol/README.md) — Protocol overview
