# Tanren Shared Command Sources

This directory is the single source of truth for Tanren's shared agent
commands. `tanren-cli install` renders these sources into per-agent-framework
destinations.

Do not hand-edit rendered artifacts. Edit files in `commands/` and re-run
`just install-commands` in this repository, or `tanren-cli install` in an
adopting repository.

## Install Contract Reference

Use `tanren-cli install --profile <PROFILE> [--repo <PATH>] [--integrations <CSV>]`
to materialize command assets in a target repository.

## Read-Only Drift Entry Point

Use `tanren-cli drift --profile <PROFILE> [--repo <PATH>] [--integrations <CSV>]`
to analyze install-managed assets without writing files.

This README does not define the stable R-0024 drift output contract, exit
behavior, or ownership boundary. Those are canonical in
[Delivery Architecture](../docs/architecture/delivery.md). This directory
remains the source of command content that install materializes into
harness-specific paths.

Lifecycle and ownership rules are canonical in
[`docs/architecture/delivery.md`](../docs/architecture/delivery.md); this
directory does not redefine install/preview/drift/upgrade/uninstall semantics.

Reference sections:

- [Generated Repository Assets](../docs/architecture/delivery.md#generated-repository-assets)
- [CLI And TUI Role](../docs/architecture/delivery.md#cli-and-tui-role)
- [Current Read-Only Drift Command Surface (R-0024)](../docs/architecture/delivery.md#current-read-only-drift-command-surface-r-0024)
- [Projection Drift](../docs/architecture/delivery.md#projection-drift)
- [Install Preview](../docs/architecture/delivery.md#install-preview)
- [Upgrades And Migrations](../docs/architecture/delivery.md#upgrades-and-migrations)
- [Stack Uninstall](../docs/architecture/delivery.md#stack-uninstall)
- [Repo Uninstall](../docs/architecture/delivery.md#repo-uninstall)

> **Note (rewrite reset):** the `spec/` directory and the
> `assess-implementation` command have been removed during the architecture
> rewrite. The spec-orchestration state machine is being redesigned from
> scratch and will be reintroduced as Tanren-native, typed-event-driven
> commands. Until then, only the four project-method commands below are
> supported.

## Layout

- `project/` contains the project-method commands that drive the planning
  loop end-to-end. They are installed prompts, not yet native typed
  orchestration phases.

Current project-method chain:

```text
plan-product
-> identify-behaviors
-> architect-system
-> craft-roadmap
```

Project commands directly edit owned planning projections for now:

- `plan-product` owns `docs/product/**`.
- `identify-behaviors` owns `docs/behaviors/**`.
- `architect-system` owns `docs/architecture/**`.
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
declared-but-unused variables, and referenced-but-undeclared variables are
hard errors.

## Related Docs

- `docs/architecture/`
- `tests/bdd/README.md`
