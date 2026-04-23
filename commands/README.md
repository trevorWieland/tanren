# Tanren Shared Command Sources

This directory is the **single source of truth** for tanren's shared
agent commands. `tanren install` renders these into per-agent-
framework destinations (`.claude/commands/`, `.codex/skills/`,
`.opencode/commands/`).

Do not hand-edit the rendered artifacts — fork this source instead
and re-run `tanren install` (see
[docs/methodology/commands-install.md](../docs/methodology/commands-install.md)).

## Layout

- **[spec commands](spec/shape-spec.md)** — commands that participate in the
  spec-orchestration state machine. Each emits typed events via the
  agent tool surface and contributes to task / finding state.
- **[project commands](project/plan-product.md)** — project-management commands that
  operate outside the spec loop. Still templated and installed, but
  not sequenced by the state machine.

## Authoring contract

Every file follows the uniform skeleton:

```markdown
---
name: <command>
role: conversation | implementation | audit | adherence | feedback | meta | triage
orchestration_loop: true | false
autonomy: interactive | autonomous
declared_variables: [...]
declared_tools: [...]
required_capabilities: [...]
produces_evidence: [...]
---

# <Title>

## Purpose
<opinionated paragraph>

## Inputs (from your dispatch)
<abstract references>

## Responsibilities
<directive prose>

## Verification
Run `{{TASK_VERIFICATION_HOOK}}` (or phase-specific variant).

## Emitting results
{{TASK_TOOL_BINDING}}
{{READONLY_ARTIFACT_BANNER}}

## Out of scope
<uniform list: issue/branch/commit/PR ops; artifact edits;
 phase/gate selection>
```

Template variables (`{{UPPER_SNAKE}}`) are filled install-time from
`tanren.yml` + the rubric/standards profile. Unknown variables are
hard errors at install time. Variables declared but never
referenced, or referenced but not declared, are hard errors too.

## Template variable taxonomy

Full table in
[docs/architecture/install-targets.md](../docs/architecture/install-targets.md).
Key entries:

| Variable | Purpose |
|---|---|
| `{{TASK_VERIFICATION_HOOK}}` / `{{SPEC_VERIFICATION_HOOK}}` + per-phase overrides | Resolved gate command for the phase |
| `{{ISSUE_PROVIDER}}`, `{{ISSUE_REF_NOUN}}`, `{{PR_NOUN}}` | Display nouns (GitHub/Linear/…) |
| `{{SPEC_ROOT}}`, `{{PRODUCT_ROOT}}`, `{{STANDARDS_ROOT}}` | Directory roots |
| `{{PROJECT_LANGUAGE}}` | Agent hint |
| `{{TASK_TOOL_BINDING}}` | MCP tool-call prose or CLI-command prose |
| `{{READONLY_ARTIFACT_BANNER}}` | Three-layer read-only warning |

## Agent tool surface

Every structured mutation passes through typed tools (MCP or CLI
fallback). Full catalog:
[docs/architecture/agent-tool-surface.md](../docs/architecture/agent-tool-surface.md).

Each command declares the tools it calls and the phase capabilities
it requires. Out-of-scope calls are rejected at dispatch with
`CapabilityDenied`.

## Related docs

- [Orchestration flow](../docs/architecture/orchestration-flow.md)
- [Methodology boundary](../docs/rewrite/METHODOLOGY_BOUNDARY.md)
- [Evidence schemas](../docs/architecture/evidence-schemas.md)
- [Audit rubric](../docs/architecture/audit-rubric.md)
- [Adherence](../docs/architecture/adherence.md)
- [Install targets](../docs/architecture/install-targets.md)
- [Lane 0.5 design notes](../docs/rewrite/tasks/LANE-0.5-DESIGN-NOTES.md)
