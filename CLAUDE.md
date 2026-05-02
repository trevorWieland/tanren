# Claude Working Notes for Tanren

Tanren is a Rust control plane for agentic software delivery. The canonical
architecture lives in [`docs/architecture/`](docs/architecture/); the general
contributor guide is [`AGENTS.md`](AGENTS.md). This file captures
**Claude-specific** working guidance only — it does not restate technical
detail that belongs in those records.

When in doubt, read the architecture record before editing code or docs.

## Source-of-truth map

| Question | Read this first |
|---|---|
| What is Tanren for? | [`docs/product/vision.md`](docs/product/vision.md) |
| What can a user do? | [`docs/behaviors/`](docs/behaviors/) and [`docs/behaviors/index.md`](docs/behaviors/index.md) |
| How is the system structured? | [`docs/architecture/system.md`](docs/architecture/system.md) |
| What technology + workspace conventions? | [`docs/architecture/technology.md`](docs/architecture/technology.md) |
| Subsystem ownership and contracts | [`docs/architecture/subsystems/`](docs/architecture/subsystems/) |
| Operations + delivery posture | [`docs/architecture/operations.md`](docs/architecture/operations.md), [`docs/architecture/delivery.md`](docs/architecture/delivery.md) |
| What work is planned and in what order? | [`docs/roadmap/dag.json`](docs/roadmap/dag.json) (canonical), [`docs/roadmap/roadmap.md`](docs/roadmap/roadmap.md) (rendered) |

If a technical question is not answered by the docs above, that is a real
gap to be raised through the `architect-system` skill, not guessed at.

## Methodology — use the skills, do not freelance

Tanren is built using its own methodology. The artifacts under `docs/` are
each owned by a specific skill. When the user asks for work that fits a
skill description, invoke the skill via the **Skill** tool. Don't edit those
artifacts directly except for explicit out-of-band fixes (audit cleanup, ID
renames, tooling repairs).

| Artifact | Owning skill |
|---|---|
| `docs/product/**` | `plan-product` |
| `docs/behaviors/**` | `identify-behaviors` |
| `docs/architecture/**` | `architect-system` |
| `docs/roadmap/**` | `craft-roadmap` |
| Implementation assessment | `assess-implementation` |

The roadmap DAG is the bridge between behaviors and architecture. Any change
to behaviors or architecture should propagate through the DAG via
`craft-roadmap`; the validator [`scripts/roadmap_check.py`](scripts/roadmap_check.py)
catches drift mechanically.

## Foundation state (current branch)

The workspace is mid-F-0001. **F-0001** in
[`docs/roadmap/dag.json`](docs/roadmap/dag.json) is the foundation spec that
brings the workspace from "scaffolding only" to "minimum buildable Tanren":
every subsystem stubbed, every public interface (web, api, mcp, cli, tui)
hosting a runnable scaffold, and the BDD harness wired into `just tests`.

Do not attempt to add behavior implementation before F-0001 lands — the
crate skeletons, interface scaffolding, and BDD machinery that every
behavior slice depends on come from F-0001.

## Workflow rules

- **Run gates through `just`.** Never substitute raw `cargo` calls when a
  recipe exists. The full PR gate is `just ci`. Recipe list: `just --list`.
- **Mutation testing is nightly-only.** `just mutation` runs as a scheduled
  main-branch job that uploads failure artifacts; it is intentionally NOT
  part of `just ci`. Don't recommend wiring it into CI.
- **Tests live exclusively in BDD.** All scenarios live under
  [`tests/bdd/features/`](tests/bdd/) and step definitions in the
  `tanren-bdd` crate. No `#[cfg(test)]` modules or `#[test]` functions
  outside the BDD crate — `xtask check-rust-test-surface` enforces this.
- **Follow the BDD convention.** One `.feature` per behavior, named
  `B-XXXX-<slug>.feature`; closed tag allowlist; strict per-interface
  positive + falsification coverage. Canonical contract:
  [`docs/architecture/subsystems/behavior-proof.md`](docs/architecture/subsystems/behavior-proof.md)
  ("BDD Tagging And File Convention"). Enforced by
  `xtask check-bdd-tags` (wired into `just check`).
- **No inline lint suppressions.** Workspace policy denies `allow_attributes`
  and `allow_attributes_without_reason`. Relax a lint in the owning crate's
  `[lints.clippy]` section with a comment explaining why; never use inline
  `#[allow(...)]` or `#[expect(...)]`.
- **Crate dependency rules are mechanically enforced.** See the dependency
  rules in [`docs/architecture/technology.md`](docs/architecture/technology.md);
  `xtask check-deps` validates them. Don't add a dependency edge that crosses
  a stated boundary without raising an architecture decision.
- **Cite IDs.** Reference behaviors as `B-XXXX`, roadmap nodes as `R-XXXX`,
  milestones as `M-XXXX`, foundation as `F-XXXX`. The DAG validator and
  human readers both rely on these stable IDs.

## Validating roadmap and behavior edits

After any change to `docs/roadmap/dag.json` or `docs/behaviors/B-*.md`, run
`python3 scripts/roadmap_check.py`. It enforces:

- structural validity and acyclicity;
- every accepted behavior is completed by exactly one node;
- every behavior node transitively depends on the current
  `foundation_spec_id` (F-0002 since the foundation correction landed);
- evidence-item interfaces match the behavior frontmatter (no drift);
- every `tests/bdd/features/B-XXXX-*.feature` file maps to an accepted
  behavior with a DAG node;
- no transitively redundant `depends_on` edges.

The validator also surfaces a non-blocking warning for nodes whose playbook
is suspiciously thin relative to declared interfaces. That warning indicates
a future `shape-spec` follow-up, not a blocker.

## Branch and commit conventions

See [`AGENTS.md`](AGENTS.md) for Conventional Commit conventions, PR
expectations, and the role of `just ci`. Don't bypass pre-commit hooks
(`lefthook.yml`) — fix the underlying issue.

## What this file is NOT

This file is not a duplicate of architecture, technology, or behavior
contracts. If a technical claim here contradicts a record under `docs/`, the
record under `docs/` wins and this file should be corrected.
