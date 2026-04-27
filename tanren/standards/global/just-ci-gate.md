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

**Why:** A single broken gate means broken code reaches the main branch. Running all gates locally with `just ci` before push catches issues before CI, keeping feedback loops fast.
