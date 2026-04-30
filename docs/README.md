# Tanren-Owned Documentation

`docs/` is not a scratch documentation folder. It is the committed projection
of Tanren-owned planning state for this repository.

Every durable document in this tree has an owner in the Tanren method. Temporary
analysis, run logs, prompts, partial reports, and raw event streams belong under
`artifacts/` or Tanren's database-backed event store, not in `docs/`.

## Method Chain

```text
plan-product
-> identify-behaviors
-> architect-system
-> assess-implementation
-> craft-roadmap
-> shape-spec / orchestrate / walk
```

Product and behavior documents preserve intent. Architecture, implementation
state, and roadmap documents can be regenerated as the system, proof state,
source signals, and delivery plan change.

## Ownership Map

| Path | Owner | Purpose |
|---|---|---|
| `product/vision.md` | `plan-product` | Product identity, purpose, users, constraints, non-goals, success signals, and open product decisions. |
| `product/personas.md` | `plan-product` | Canonical user, operator, observer, and client personas referenced by behaviors. |
| `product/concepts.md` | `plan-product` | Product vocabulary and high-level concepts used by behavior and roadmap work. |
| `behaviors/index.md` | `identify-behaviors` | Behavior authoring rules and generated catalog index. |
| `behaviors/B-*.md` | `identify-behaviors` | One accepted, draft, deprecated, or removed behavior per file. |
| `architecture/system.md` | `architect-system` | High-level system architecture and boundaries. |
| `architecture/technology.md` | `architect-system` | Language, workspace, build, test, and toolchain decisions. |
| `architecture/delivery.md` | `architect-system` | Installation, command rendering, MCP setup, distribution, and delivery posture. |
| `architecture/operations.md` | `architect-system` | Security, secrets, standards, policy, audits, and operational posture. |
| `architecture/subsystems/*.md` | `architect-system` | Focused architecture records for project-specific subsystems. |
| `implementation/readiness.json` | `assess-implementation` | Machine-readable implementation-readiness summary. |
| `implementation/readiness.md` | `assess-implementation` | Human-readable implementation-readiness projection. |
| `implementation/verification.md` | `assess-implementation` | Current behavior verification classification. |
| `roadmap/dag.json` | `craft-roadmap` | Machine-readable roadmap DAG source of truth. |
| `roadmap/roadmap.md` | `craft-roadmap` | Human-readable projection of the roadmap DAG. |

## Boundary Rules

- Do not add arbitrary markdown files under `docs/`.
- Do not store raw event JSONL, temporary reports, or agent scratch notes here.
- Do not put implementation proof or source references inside behavior files;
  implementation, BDD, roadmap, or spec artifacts cite behavior IDs.
- Do not make roadmap prose the planning source of truth; use `roadmap/dag.json`.
- Folder-level README files should be generated projections only. This file is
  the repository-level ownership index.

## External Proof And Runtime Artifacts

- BDD proof scenarios live under `tests/bdd/`.
- Active spec execution artifacts live under the configured spec root.
- Temporary readiness runs live under `artifacts/behavior/readiness/`.
- Installed agent command projections live outside `docs/` in `.claude/`,
  `.codex/`, and `.opencode/`; their source is `commands/`.
