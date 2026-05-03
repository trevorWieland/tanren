---
kind: standard
name: just-ci-gate
category: global
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains: []
---

# Just CI Gate

`just ci` is the single verification gate. All sub-gates must pass before merge. No exceptions, no overrides.

```bash
# ✓ Good: Full gate check before merge
just ci   # Runs all gates in sequence

# Individual gates:
just fmt                 # Rust formatting + TOML formatting
just lint                # Clippy with -D warnings
just check               # cargo check --workspace --all-targets
just check-lines         # Max 500 lines per .rs file
just check-suppression   # No inline #[allow/expect]
just check-deps          # Crate layering rules
just deny                # License + advisory audit
just test                # cargo nextest run
just machete             # Unused dependency detection
just doc                 # Build docs with -D warnings
```

```bash
# ✗ Bad: Skipping gates or running partial checks
cargo test               # Wrong test runner
cargo clippy || true     # Silencing lint errors
just test && just lint   # Missing other gates
```

**Task runner:**
- Use `just` (not `make`) — cross-platform, declarative, no hidden state
- Shell: `bash -euo pipefail` for fail-fast behavior
- All recipes defined in `justfile` at workspace root

**Pre-commit hooks (lefthook):**
- `cargo fmt --check` — formatting
- `cargo clippy --workspace --all-targets --quiet -- -D warnings` — linting
- `taplo fmt --check <explicit-glob-list>` — TOML formatting
- All three run in parallel via `lefthook.yml`

**Bootstrap:**
- `just bootstrap` installs all tools via `cargo-binstall` (with fallback to `cargo install --locked`)
- Tools: cargo-nextest, cargo-deny, cargo-llvm-cov, cargo-machete, cargo-hack, cargo-insta, taplo, lefthook

**CI pipeline (GitHub Actions):**
- **check** job: fmt + taplo + clippy (ubuntu + macos matrix)
- **test** job: nextest with `--profile ci` (ubuntu + macos matrix)
- **coverage** job: llvm-cov to codecov (ubuntu)
- **deny** job: cargo-deny-action (ubuntu)
- **doc** job: `RUSTDOCFLAGS="-D warnings"` (ubuntu)
- **machete** job: unused deps (ubuntu)
- **quality-gates** job: check-lines + check-suppression + check-deps (ubuntu)

## R-0001 enforcement recipes

R-0001 introduces a family of `just check-*` recipes — each backed by a
small `xtask` AST walker or scanner — that catch architectural drift the
compiler cannot see. They are composed by `just check`, which is itself
part of `just ci`. Lefthook pre-commit runs the fast subset.

| Recipe | Enforces |
|---|---|
| `just check-secrets` | No raw secret literals or unwrapped secret types in code or fixtures |
| `just check-bdd-wire-coverage` | BDD step bodies do not call `Handlers::` directly — every interface witness routes through a per-interface harness |
| `just check-test-hooks` | No `#[cfg(test)]` or `#[test]` outside the BDD crate; covers the rust-test-surface invariant for R-0001 additions |
| `just check-newtype-ids` | Bare `uuid::Uuid` field types are rejected in `tanren-{contract,store,identity-policy,app-services}` outside the newtype declaration site |
| `just check-tracing-init` | Every `bin/*/src/main.rs` calls `tanren_observability::init(env_filter)` before any other work |
| `just check-event-coverage` | Every CLI/MCP/API contract event identifier listed in the contract has at least one emitter and one consumer test path |
| `just check-profiles` | Standards profiles stay structurally valid (frontmatter, schema, no orphan files) |
| `just check-orphan-traits` | No public traits without at least one implementor or one mocked usage in a step |
| `just check-thin-binary` | `bin/*/src/main.rs` only wires deps + dispatches — no business logic in binary crates |
| `just check-tsconfig` | `apps/web` TS config matches the workspace's standardized strictness settings |
| `just check-enforcement-regressions` | Each enforcement check has a falsification fixture proving it actually fails when violated |

The orchestrating `check` recipe runs all of them. CI runs `just ci`,
which calls `just check` plus the existing fmt/lint/test/deny/doc/machete
gates. Lefthook pre-commit runs the fast subset — formatting, clippy,
TOML formatting, and the cheapest of the `check-*` recipes — so push
feedback stays under the seconds-budget.

**Why:** A single broken gate means broken code reaches the main branch. Running all gates locally with `just ci` before push catches issues before CI, keeping feedback loops fast.
