# Tanren Shared Command Sources

This directory is the single source of truth for Tanren's shared agent
commands. `tanren-cli install` renders these sources into per-agent-framework
destinations such as `.claude/commands/`, `.codex/skills/`, and
`.opencode/commands/`.

Do not hand-edit rendered artifacts. Edit files in `commands/` and re-run
`just install-commands` in this repository, or `tanren-cli install` in an
adopting repository.

## Layout

- `spec/` contains commands that participate in the spec-orchestration state
  machine. These commands emit typed events through the agent tool surface and
  contribute to task, finding, evidence, and phase state.
- `project/` contains temporary project-method commands. They are installed
  prompts, but they are not native typed orchestration phases yet.

Current project-method chain:

```text
plan-product
-> identify-behaviors
-> architect-system
-> assess-implementation
-> craft-roadmap
-> shape-spec / orchestrate / walk
```

Project commands directly edit owned planning projections for now:

- `plan-product` owns `docs/product/**`.
- `identify-behaviors` owns `docs/behaviors/**`.
- `architect-system` owns `docs/architecture/**`.
- `assess-implementation` owns `docs/implementation/**`.
- `craft-roadmap` owns `docs/roadmap/**`.

These commands should later be replaced by Tanren-native commands backed by
typed schemas, validators, tools, and project-method events.

## Authoring Contract

Every source command uses YAML frontmatter plus markdown body:

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
```

Template variables (`{{UPPER_SNAKE}}`) are filled at install time from
`tanren.yml` and standards/rubric configuration. Unknown variables,
declared-but-unused variables, and referenced-but-undeclared variables are hard
errors.

## Tool Surface

Spec-loop commands mutate structured state through typed tools exposed by MCP
or CLI fallback. The canonical tool and capability contract is documented in
`docs/architecture/subsystems/tools.md`.

Project-method commands do not yet have typed project tools. They must keep
edits small, structured, and projection-friendly so their artifacts can migrate
to native Tanren storage later.

## Related Docs

- `docs/README.md`
- `docs/architecture/delivery.md`
- `docs/architecture/subsystems/orchestration.md`
- `docs/architecture/subsystems/tools.md`
- `docs/architecture/subsystems/evidence.md`
- `docs/architecture/subsystems/audit.md`
- `docs/architecture/subsystems/adherence.md`
- `docs/roadmap/roadmap.md`
