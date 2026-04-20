# Tanren — project commands
#
# We use `just` (https://github.com/casey/just) instead of Make because:
# - No tab-vs-space footguns — just uses normal indentation
# - No $$ escaping — shell variables work naturally
# - Native recipe arguments — `just test --filter foo` works directly
# - Cross-platform — runs natively on Linux, macOS, and Windows
# - Built-in `just --list` — no DIY help target needed
# - Cleaner conditionals — no awkward shell-in-make blocks
#
# Install: cargo binstall just
# Usage:  just --list

# Default recipe: show available commands
default:
    @just --list

# ============================================================================
# Settings
# ============================================================================

set shell := ["bash", "-euo", "pipefail", "-c"]

cargo := env("CARGO", "cargo")
max_lines := "500"
nextest_quiet_flags := "--status-level fail --final-status-level fail --success-output never --failure-output immediate-final --cargo-quiet"

# ============================================================================
# Setup
# ============================================================================

# Install all external tooling (first-time setup, idempotent)
bootstrap:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "==> Checking for rustup..."
    if ! command -v rustup &>/dev/null; then
        echo "Installing rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        echo "Run 'source ~/.cargo/env' or restart your shell, then re-run 'just bootstrap'"
        exit 1
    fi

    echo "==> Ensuring stable toolchain with components..."
    rustup show active-toolchain &>/dev/null || rustup default stable
    rustup component add rustfmt clippy llvm-tools-preview 2>/dev/null || true

    echo "==> Installing cargo-binstall..."
    if ! command -v cargo-binstall &>/dev/null; then
        curl -L --proto '=https' --tlsv1.2 -sSf \
            https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
    fi

    echo "==> Installing cargo tools (skipping already-installed)..."
    failed=""

    # cargo-nextest: use official installer (binstall often fails to compile from source)
    if ! command -v cargo-nextest &>/dev/null; then
        echo "  Installing cargo-nextest..."
        nextest_platform="linux"
        [[ "$(uname -s)" == "Darwin" ]] && nextest_platform="mac"
        if ! curl -LsSf "https://get.nexte.st/latest/${nextest_platform}" | tar zxf - -C "${CARGO_HOME:-$HOME/.cargo}/bin" 2>/dev/null; then
            if ! cargo binstall --no-confirm cargo-nextest; then
                echo "  FAIL: cargo-nextest"
                failed="$failed cargo-nextest"
            fi
        fi
    fi

    # Remaining tools via binstall (install one at a time so one failure doesn't abort the rest)
    tools=(
        "cargo-deny:cargo-deny"
        "cargo-llvm-cov:cargo-llvm-cov"
        "cargo-machete:cargo-machete"
        "cargo-hack:cargo-hack"
        "cargo-insta:cargo-insta"
        "taplo:taplo-cli"
        "just:just"
    )
    for entry in "${tools[@]}"; do
        bin="${entry%%:*}"
        pkg="${entry##*:}"
        if ! command -v "$bin" &>/dev/null; then
            echo "  Installing $pkg..."
            if ! cargo binstall --no-confirm "$pkg"; then
                echo "  FAIL: $pkg"
                failed="$failed $pkg"
            fi
        fi
    done

    echo "==> Platform-specific setup..."
    if [[ "$(uname -s)" == "Linux" ]]; then
        if command -v apt-get &>/dev/null; then
            echo "Installing mold linker (apt)..."
            sudo apt-get install -y mold clang 2>/dev/null || echo "  (skipped — install mold manually if needed)"
        elif command -v dnf &>/dev/null; then
            echo "Installing mold linker (dnf)..."
            sudo dnf install -y mold clang 2>/dev/null || echo "  (skipped — install mold manually if needed)"
        else
            echo "  Install mold linker manually: https://github.com/rui314/mold"
        fi
    else
        echo "  macOS/Windows detected — using default linker (no mold needed)"
    fi

    echo "==> Installing lefthook..."
    if ! command -v lefthook &>/dev/null; then
        # Official install script works on Linux, macOS, and WSL — no brew/go required
        if curl -1sLf 'https://dl.cloudsmith.io/public/evilmartians/lefthook/setup.shell.sh' | bash 2>/dev/null \
            && command -v lefthook &>/dev/null; then
            true
        elif command -v brew &>/dev/null; then
            brew install lefthook
        else
            echo "  FAIL: lefthook"
            failed="$failed lefthook"
        fi
    fi

    if command -v lefthook &>/dev/null; then
        echo "==> Activating git hooks..."
        lefthook install
    fi

    if [[ -n "$failed" ]]; then
        echo ""
        echo "==> Bootstrap completed with failures:"
        echo "    Failed to install:$failed"
        echo "    Install these manually, then re-run 'just bootstrap' to verify."
        exit 1
    fi

    echo "==> Bootstrap complete!"

# Fetch dependencies and verify build
install:
    {{ cargo }} fetch
    {{ cargo }} build --workspace

# ============================================================================
# Methodology self-hosting (tanren-repo specific)
#
# Dogfoods the methodology installer: renders `commands/` + the
# bundled baseline standards into the three agent targets and writes
# the MCP server registration into each agent's config. Adopters need
# not replicate these recipes — `tanren install` is the single
# public-facing entry point; these are convenience wrappers over it
# for local development and CI drift-detection on the tanren repo
# itself.
# ============================================================================

# Render the command catalog and standards per `tanren.yml`.
install-commands:
    @{{ cargo }} run --quiet -p tanren-cli -- install

# Preview installer writes without mutating files.
install-commands-dry-run:
    @{{ cargo }} run --quiet -p tanren-cli -- install --dry-run

# Strict dry-run: fail if rendered artifacts drift from the plan.
# Wired into `just ci` below so merging a command-source change
# without re-running `install-commands` fails at PR time.
#
# Captures install stdout/stderr so the happy path is silent; on
# drift (exit 3) or any other failure the full plan + error are
# replayed to the console for triage.
install-commands-check:
    #!/usr/bin/env bash
    set -euo pipefail
    output=$({{ cargo }} run --quiet -p tanren-cli -- install --dry-run --strict 2>&1) && status=0 || status=$?
    if [[ $status -ne 0 ]]; then
        echo "$output"
        echo "FAIL: installer drift — re-run 'just install-commands' and commit the result." >&2
        exit "$status"
    fi

# ============================================================================
# Build
# ============================================================================

# Build all workspace crates
build:
    @{{ cargo }} build --workspace --quiet

# Type-check all workspace crates
check:
    @{{ cargo }} check --workspace --all-targets --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --quiet

# ============================================================================
# Test
# ============================================================================

# Run all tests via nextest (pass extra args after --)
test *args:
    @{{ cargo }} nextest run --workspace --no-tests=pass --features tanren-store/test-hooks,tanren-orchestrator/test-hooks {{ nextest_quiet_flags }} {{ args }}

# Generate code coverage report (lcov)
coverage:
    @{{ cargo }} llvm-cov nextest --workspace --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --lcov --output-path lcov.info --no-tests=pass
    @echo "Coverage report: lcov.info"

# ============================================================================
# Lint & Format
# ============================================================================

# Run clippy with deny warnings
lint:
    @{{ cargo }} clippy --workspace --all-targets --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --quiet -- -D warnings

# Glob for Rust workspace TOML files (excludes Python pyproject.toml)
toml_globs := "Cargo.toml bin/*/Cargo.toml crates/*/Cargo.toml .cargo/*.toml .config/*.toml rust-toolchain.toml clippy.toml taplo.toml deny.toml .rustfmt.toml lefthook.yml"

# Check formatting (Rust + TOML)
fmt:
    @{{ cargo }} fmt --check
    @RUST_LOG=error taplo fmt --check {{ toml_globs }}

# Auto-fix formatting (Rust + TOML)
fmt-fix:
    @{{ cargo }} fmt
    @RUST_LOG=error taplo fmt {{ toml_globs }}

# Auto-fix everything that can be auto-fixed (formatting + clippy suggestions)
fix:
    @{{ cargo }} fmt
    @RUST_LOG=error taplo fmt {{ toml_globs }}
    @{{ cargo }} clippy --workspace --all-targets --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --fix --allow-dirty --allow-staged --quiet -- -D warnings

# ============================================================================
# Audit & Analysis
# ============================================================================

# Audit dependencies (licenses, advisories, bans, sources)
deny:
    @{{ cargo }} deny --log-level error check

# Detect unused dependencies
machete:
    @{{ cargo }} machete

# ============================================================================
# Documentation
# ============================================================================

# Build documentation (warnings are errors)
doc:
    @RUSTDOCFLAGS="-D warnings" {{ cargo }} doc --workspace --no-deps --quiet

# ============================================================================
# Quality Gates
# ============================================================================

# Enforce max file line count (500 lines per .rs file)
check-lines:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Checking .rs files for line count > {{ max_lines }}..."
    failed=0
    while IFS= read -r -d '' file; do
        lines=$(wc -l < "$file")
        if [[ "$lines" -gt {{ max_lines }} ]]; then
            echo "FAIL: $file has $lines lines (max {{ max_lines }})"
            failed=1
        fi
    done < <(find crates/ bin/ -name '*.rs' -print0)
    if [[ "$failed" -eq 1 ]]; then exit 1; fi
    echo "All files within line limit."

# Enforce crate dependency layering (foundation crates must not depend on capability crates)
check-deps:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Checking crate dependency layering..."

    # Foundation crates: domain logic, no I/O, no orchestration
    foundation=(
        "tanren-domain"
        "tanren-contract"
        "tanren-policy"
        "tanren-observability"
    )

    # Capability crates: these must NOT be depended upon by foundation crates
    capability=(
        "tanren-store"
        "tanren-planner"
        "tanren-scheduler"
        "tanren-orchestrator"
        "tanren-app-services"
        "tanren-runtime"
        "tanren-runtime-local"
        "tanren-runtime-docker"
        "tanren-runtime-remote"
        "tanren-harness-claude"
        "tanren-harness-codex"
        "tanren-harness-opencode"
    )

    failed=0
    metadata=$({{ cargo }} metadata --format-version 1 --no-deps 2>/dev/null)

    # Normal (non-dev, non-build) dependencies only. `cargo metadata`
    # tags dev-dependencies with `kind="dev"` — those are test-only
    # and never land in the shipped binary, so they do not constitute
    # a layering violation.
    for fnd in "${foundation[@]}"; do
        deps=$(echo "$metadata" \
            | jq -r ".packages[] | select(.name == \"$fnd\") | .dependencies[] | select(.kind == null) | .name" 2>/dev/null || true)
        for cap in "${capability[@]}"; do
            if echo "$deps" | grep -qx "$cap"; then
                echo "FAIL: foundation crate '$fnd' depends on capability crate '$cap'"
                failed=1
            fi
        done
    done

    # Transport binaries must stay thin: no direct core/capability deps.
    transport=(
        "tanren-cli"
        "tanren-api"
        "tanren-mcp"
        "tanren-tui"
    )
    forbidden_transport=(
        "tanren-domain"
        "tanren-policy"
        "tanren-store"
        "tanren-planner"
        "tanren-scheduler"
        "tanren-orchestrator"
    )
    for bin in "${transport[@]}"; do
        deps=$(echo "$metadata" \
            | jq -r ".packages[] | select(.name == \"$bin\") | .dependencies[] | select(.kind == null) | .name" 2>/dev/null || true)
        for forbidden in "${forbidden_transport[@]}"; do
            if echo "$deps" | grep -qx "$forbidden"; then
                echo "FAIL: transport binary '$bin' depends directly on '$forbidden'"
                failed=1
            fi
        done
    done

    # Store row-shape entities must remain crate-internal.
    if grep -Eq '^[[:space:]]*pub mod entity;' crates/tanren-store/src/lib.rs; then
        echo "FAIL: crates/tanren-store/src/lib.rs exports 'pub mod entity;'"
        failed=1
    fi
    if grep -Eq '^[[:space:]]*pub mod (dispatch_projection|events|step_projection);' crates/tanren-store/src/entity/mod.rs; then
        echo "FAIL: crates/tanren-store/src/entity/mod.rs exposes row-shape modules publicly"
        failed=1
    fi
    if ! awk '
        prev_cfg && /^[[:space:]]*pub use state_store::dispatch_query_statement_for_backend;/ {ok=1}
        { prev_cfg = ($0 ~ /^[[:space:]]*#\[cfg\(feature = "test-hooks"\)\][[:space:]]*$/) }
        END { exit ok ? 0 : 1 }
    ' crates/tanren-store/src/lib.rs; then
        echo "FAIL: dispatch_query_statement_for_backend re-export must be gated by #[cfg(feature = \"test-hooks\")]"
        failed=1
    fi
    if ! awk '
        prev_cfg && /^[[:space:]]*pub fn dispatch_query_statement_for_backend\(/ {ok=1}
        { prev_cfg = ($0 ~ /^[[:space:]]*#\[cfg\(feature = "test-hooks"\)\][[:space:]]*$/) }
        END { exit ok ? 0 : 1 }
    ' crates/tanren-store/src/state_store.rs; then
        echo "FAIL: dispatch_query_statement_for_backend function must be gated by #[cfg(feature = \"test-hooks\")]"
        failed=1
    fi

    if [[ "$failed" -eq 1 ]]; then
        echo "Dependency/boundary rule violations detected."
        exit 1
    fi
    echo "Crate layering rules pass."

# Verify local CI recipes stay aligned with workflow strict rust commands.
check-ci-parity:
    #!/usr/bin/env bash
    set -euo pipefail

    uv_bin="$(command -v uv || true)"
    if [[ -z "$uv_bin" && -x "$HOME/.local/bin/uv" ]]; then
        uv_bin="$HOME/.local/bin/uv"
    fi
    if [[ -z "$uv_bin" ]]; then
        echo "FAIL: uv not found. Install uv or add it to PATH."
        exit 127
    fi

    # Keep cache local when HOME cache is unavailable (sandboxed runs).
    export UV_CACHE_DIR="${UV_CACHE_DIR:-$PWD/.uv-cache}"
    "$uv_bin" run python scripts/check_ci_parity.py

# Prohibit inline lint suppression (#[allow/expect])
check-suppression:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Checking for inline lint suppression..."
    found=0
    if grep -rn '#\[allow(' crates/ bin/ --include='*.rs' 2>/dev/null; then
        echo "FAIL: Found #[allow(...)] in source. Move to [lints] in Cargo.toml."
        found=1
    fi
    if grep -rn '#\[expect(' crates/ bin/ --include='*.rs' 2>/dev/null; then
        echo "FAIL: Found #[expect(...)] in source. Move to [lints] in Cargo.toml."
        found=1
    fi
    if grep -rn '#!\[allow(' crates/ bin/ --include='*.rs' 2>/dev/null; then
        echo "FAIL: Found #![allow(...)] in source. Move to [lints] in Cargo.toml."
        found=1
    fi
    if [[ "$found" -eq 1 ]]; then exit 1; fi
    echo "No inline lint suppression found."

# ============================================================================
# Benchmarks
# ============================================================================

# Run benchmarks
bench:
    {{ cargo }} bench --workspace

# ============================================================================
# Maintenance
# ============================================================================

# Remove build artifacts
clean:
    {{ cargo }} clean

# ============================================================================
# CI
# ============================================================================

# Run workflow-equivalent strict Rust checks locally.
ci-rust-strict:
    @echo "==> Clippy"
    @RUSTFLAGS="-D warnings" {{ cargo }} clippy --workspace --all-targets --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --quiet -- -D warnings
    @echo "==> Workspace tests"
    @RUSTFLAGS="-D warnings" {{ cargo }} nextest run --workspace --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --profile ci --no-tests=pass {{ nextest_quiet_flags }}
    @echo "==> Postgres integration"
    @./scripts/run_postgres_integration.sh

# Run full CI check locally.
ci:
    @echo "==> Format"
    @just fmt
    @echo "==> File length guard"
    @just check-lines
    @echo "==> Lint suppression guard"
    @just check-suppression
    @echo "==> Dependency layering guard"
    @just check-deps
    @echo "==> CI parity guard"
    @just check-ci-parity
    @echo "==> Dependency audit"
    @just deny
    @echo "==> Docs"
    @just doc
    @echo "==> Unused dependency audit"
    @just machete
    @echo "==> Strict Rust CI"
    @just ci-rust-strict
    @echo "==> Installer drift guard"
    @just install-commands-check
    @echo "==> All CI checks passed!"
