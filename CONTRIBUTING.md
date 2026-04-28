# Contributing

All commands run from the repository root.

## Development Setup

```bash
just bootstrap
just install
just check
```

Run full validation before opening or updating a PR:

```bash
just fmt
just check
just ci
```

## Tooling

| Tool | Purpose | Config |
|------|---------|--------|
| Cargo | Build, check, docs, and dependency metadata | `Cargo.toml` |
| rustfmt | Rust formatting | `rust-toolchain.toml` |
| Clippy | Rust linting | workspace lints in `Cargo.toml` |
| cargo-deny | License/advisory/source checks | `deny.toml` |
| cargo-machete | Unused dependency checks | `Cargo.toml` files |
| cargo-mutants | Mutation proof stage | `just tests` |
| cargo-llvm-cov | Coverage classification stage | `just tests` |
| taplo | TOML formatting | `taplo.toml` |
| just | Task runner | `justfile` |

## Repository Areas

- `bin/`: Rust binaries
- `crates/`: Rust libraries
- `xtask/`: repo automation and proof-support commands
- `commands/`: workflow instructions rendered into agent targets
- `profiles/`: coding standards by stack
- `protocol/`: protocol overview
- `docs/`: roadmap, architecture, methodology, and operations docs
- `tests/bdd/`: behavior feature files used by the proof suite

## Commit Style

Use imperative commit subjects with Conventional Commit prefixes when possible:

- `feat(core): add ...`
- `feat(api): add ...`
- `fix: correct ...`
- `chore: update ...`

## Pull Request Requirements

Each PR should include:

- clear problem statement and implementation summary
- validation commands run and outcomes
- config/security impact notes, if applicable
- linked issue/spec IDs

## Contribution License

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this repository is licensed as `MIT OR Apache-2.0`, matching
the workspace crate metadata and repository license files.

## Documentation Update Rule

If behavior, interfaces, lifecycle, or security posture changes, update the
canonical docs in the same PR.

- Product vision: `docs/vision.md`
- Behavior canon: `docs/behaviors/`
- Roadmap DAG guidance: `docs/roadmap/`
- Architecture and boundaries: `docs/architecture/`
- Command installation and methodology: `docs/methodology/`
- Root summaries: `README.md` and `AGENTS.md`
