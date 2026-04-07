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
# Build
# ============================================================================

# Build all workspace crates
build:
    @{{ cargo }} build --workspace --quiet

# Type-check all workspace crates
check:
    @{{ cargo }} check --workspace --all-targets --quiet

# ============================================================================
# Test
# ============================================================================

# Run all tests via nextest (pass extra args after --)
test *args:
    @{{ cargo }} nextest run --workspace --no-tests=pass {{ args }}

# Generate code coverage report (lcov)
coverage:
    @{{ cargo }} llvm-cov nextest --workspace --lcov --output-path lcov.info --no-tests=pass
    @echo "Coverage report: lcov.info"

# ============================================================================
# Lint & Format
# ============================================================================

# Run clippy with deny warnings
lint:
    @{{ cargo }} clippy --workspace --all-targets --quiet -- -D warnings

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
    @{{ cargo }} clippy --workspace --all-targets --fix --allow-dirty --allow-staged --quiet -- -D warnings

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

# Run full CI check locally
ci: fmt lint deny check-lines check-suppression test doc machete
    @echo "==> All CI checks passed!"
