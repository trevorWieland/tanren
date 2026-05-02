# Repository Guidelines

Tanren is a Rust workspace. Use Cargo and `just` for all local development and
validation.

Never validate changes with targeted tests or targeted linting. Use the full
repo gates.

## Project Structure

- `bin/`: Rust binaries (`tanren-cli`, `tanren-mcp`, `tanren-api`, `tanrend`, `tanren-tui`)
- `crates/`: Rust libraries, organized by domain, contract, store, runtime, harness, and app-service boundaries
- `xtask/`: repo maintenance and proof-support commands
- `commands/`: source command markdown rendered by the installer
- `profiles/`, `protocol/`, `scripts/`: standards, protocol docs, and shell entrypoints
- `tests/bdd/`: behavior feature files used by the Rust BDD proof crate
- `docs/product/vision.md`, `docs/behaviors/`, `docs/roadmap/`: product vision,
  behavior canon, and roadmap DAG source documents

## Build, Test, and Development Commands

Run commands from the repo root.

- First-time setup: `just bootstrap`
- Install binaries locally: `just install`
- Format check: `just fmt`
- Fast static gate: `just check`
- Behavior proof suite: `just tests`
- Full PR gate: `just ci`
- Auto-fix formatting and Clippy suggestions: `just fix`

## Rust Style

- Rust edition and toolchain are pinned by the workspace.
- Public APIs use explicit types and domain newtypes where appropriate.
- Library crates use `thiserror`; binaries may use `anyhow`.
- No `unsafe`, `unwrap`, `panic!`, `todo!`, `unimplemented!`, `println!`, `eprintln!`, or `dbg!` in production code.
- No inline `#[allow]` or `#[expect]`; relax lints in the owning crate manifest with a reason.
- Keep `.rs` files under the line-count budget enforced by `just check-lines`.

## Testing Guidelines

- `just tests` is the behavior proof path; it runs the cucumber-rs BDD
  harness and the runner binary. Mutation testing is intentionally
  separated into `just mutation` and runs nightly only.
- All scenarios live under `tests/bdd/features/B-XXXX-<slug>.feature`.
  Follow the canonical convention in
  [`docs/architecture/subsystems/behavior-proof.md`](docs/architecture/subsystems/behavior-proof.md)
  ("BDD Tagging And File Convention"): one feature per behavior;
  scenario tags from a closed allowlist
  (`@positive | @falsification | @web | @api | @mcp | @cli | @tui`);
  per-interface positive + falsification coverage.
- `xtask check-bdd-tags` (wired into `just check`) hard-fails any
  feature file that strays from the convention.
- Do not add skipped or ignored behavior scenarios.

## Documentation Source of Truth

- Product vision lives in `docs/product/vision.md`; behavior canon lives under
  `docs/behaviors/`; roadmap DAG guidance lives under `docs/roadmap/`.
- Architecture details live under `docs/architecture/`.
- Methodology and command installation details live under `docs/architecture/`.
- Runtime implementation details live in `docs/architecture/subsystems/runtime.md`.
- If behavior, interfaces, lifecycle, or security model changes, update the relevant doc in the same PR.

## Commit and Pull Request Guidelines

- Use Conventional Commit prefixes such as `feat(core): ...`, `feat(api): ...`, `fix: ...`, `chore: ...`.
- Keep subjects imperative and scoped to one change set.
- `just check` and `just ci` must pass before review.
- PRs should include the problem statement, implementation summary, executed command results, and config or secret implications.
