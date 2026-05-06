---
name: define-surfaces
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
  - docs/experience/surfaces.yml
  - docs/architecture/subsystems/experience-surfaces.md
---

# define-surfaces

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes experience
surface projections directly because native surface schemas, typed tools, and
project-method events do not exist yet. Prefer stable IDs, explicit surface
kinds, and small approved edits so these artifacts can later migrate into typed
Tanren storage.

This command is for any repository adopting the Tanren method. Use the
repository's configured experience artifact path; if none is configured, use
the conventional `docs/experience/surfaces.yml` path.

## Purpose

Define the public human and machine-facing surfaces where accepted behavior can
be experienced, consumed, or proven.

A surface is a project-local experience contract. It may be a web app, command
line, TUI, game loop, desktop app, mobile app, API, chat interface, SDK,
embedded display, or another observable interaction layer. Surface IDs are not
crate names, framework names, or internal actors.

## Inputs

- Product projections from `docs/product/**`.
- Existing behavior files and `docs/behaviors/index.md`.
- Existing architecture and implementation shape.
- Current project frameworks, binaries, engines, clients, APIs, and test
  harnesses.
- Human preferences and constraints for supported devices, inputs, outputs,
  accessibility, localization, latency, and proof artifacts.

## Editable Artifacts

This command owns:

- `docs/experience/surfaces.yml`

This command may update `docs/architecture/subsystems/experience-surfaces.md`
only when the general surface model itself changes.

## Temporary Artifact Format

```yaml
schema: tanren.experience_surfaces.v0
updated_at: YYYY-MM-DD
owner_command: define-surfaces
surfaces:
  - id: terminal
    kind: command_line
    personas: []
    devices: []
    inputs: []
    outputs: []
    proof: []
```

## Responsibilities

1. Read product intent, personas, current behavior reach, and current project
   implementation before proposing surface IDs.
2. Identify every public human or machine-facing surface that should carry
   accepted behavior.
3. Choose stable, lowercase surface IDs that make sense to product and proof
   authors.
4. Classify each surface kind and record supported devices, inputs, outputs,
   accessibility expectations, and proof artifact types.
5. Distinguish public surfaces from internal actors, engines, runtimes, jobs,
   daemons, and implementation modules.
6. Preserve compatibility aliases only during an explicit migration period.
7. Summarize surface additions, removed surfaces, renamed surfaces, unsupported
   behavior reach, and proof-adapter gaps.

## Out of Scope

- Editing behavior files. Use `identify-behaviors`.
- Designing per-behavior flows and states. Use `design-experience`.
- Choosing implementation architecture. Use `architect-system`.
- Creating roadmap DAG nodes. Use `craft-roadmap`.
