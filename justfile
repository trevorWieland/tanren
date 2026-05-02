# Tanren — project commands
#
# We use `just` (https://github.com/casey/just) instead of Make because:
# - No tab-vs-space footguns — just uses normal indentation
# - No $$ escaping — shell variables work naturally
# - Native recipe arguments keep shell entrypoints explicit and discoverable
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

    expected_toolchain="$(awk -F'\"' '/^channel[[:space:]]*=/{print $2; exit}' rust-toolchain.toml)"
    if [[ -z "$expected_toolchain" ]]; then
        echo "FAIL: unable to resolve pinned toolchain from rust-toolchain.toml"
        exit 1
    fi
    echo "==> Ensuring pinned toolchain ${expected_toolchain} with components..."
    rustup show active-toolchain &>/dev/null || rustup default "${expected_toolchain}"
    rustup toolchain install "${expected_toolchain}" >/dev/null 2>&1 || true
    rustup component add rustfmt clippy llvm-tools-preview 2>/dev/null || true

    echo "==> Installing cargo-binstall..."
    if ! command -v cargo-binstall &>/dev/null; then
        curl -L --proto '=https' --tlsv1.2 -sSf \
            https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
    fi

    echo "==> Installing cargo tools (skipping already-installed)..."
    failed=""

    # Remaining tools via binstall (install one at a time so one failure doesn't abort the rest)
    tools=(
        "cargo-deny:cargo-deny"
        "cargo-llvm-cov:cargo-llvm-cov"
        "cargo-machete:cargo-machete"
        "cargo-mutants:cargo-mutants"
        "cargo-upgrade:cargo-edit"
        "cargo-hack:cargo-hack"
        "taplo:taplo-cli"
        "lychee:lychee"
        "rumdl:rumdl"
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

    echo "==> Installing actionlint..."
    if ! command -v actionlint &>/dev/null; then
        bash scripts/install-actionlint.sh || failed="$failed actionlint"
    fi

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
        if [[ "$(uname -s)" == "Linux" ]] && command -v apt-get &>/dev/null; then
            if curl -1sLf 'https://dl.cloudsmith.io/public/evilmartians/lefthook/setup.deb.sh' | sudo -E bash \
                && sudo apt-get install -y lefthook \
                && command -v lefthook &>/dev/null; then
                true
            else
                echo "  FAIL: lefthook"
                failed="$failed lefthook"
            fi
        elif command -v brew &>/dev/null; then
            if ! brew install lefthook; then
                echo "  FAIL: lefthook"
                failed="$failed lefthook"
            fi
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

# Fetch dependencies and verify build (Rust + web frontend)
install:
    @{{ cargo }} fetch --locked
    @{{ cargo }} build --workspace --locked
    @pnpm install --frozen-lockfile

# Verify lockfile and manifests are in sync without mutating Cargo.lock
deps-locked-check:
    @{{ cargo }} metadata --locked --format-version 1 --no-deps >/dev/null

# Upgrade dependency requirements to latest compatible versions, refresh lockfile, then run full CI
deps-upgrade:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v cargo-upgrade &>/dev/null; then
        echo "FAIL: cargo upgrade is unavailable. Install cargo-edit (run 'just bootstrap')." >&2
        exit 127
    fi
    {{ cargo }} upgrade
    {{ cargo }} update -w
    just ci

# Upgrade dependency requirements including major versions, refresh lockfile, then run full CI
deps-upgrade-major:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v cargo-upgrade &>/dev/null; then
        echo "FAIL: cargo upgrade is unavailable. Install cargo-edit (run 'just bootstrap')." >&2
        exit 127
    fi
    {{ cargo }} upgrade --incompatible
    {{ cargo }} update -w
    just ci

# Read-only audit of Rust + Node deps that have newer releases on
# crates.io / npm. Surfaces:
#   - Rust workspace pins that would shift under `just deps-upgrade`
#     (semver-compatible) or `just deps-upgrade-major` (incompatible).
#   - npm catalog pins (pnpm-workspace.yaml) and pnpm overrides whose
#     installed versions trail the registry's latest.
# Pure read; never mutates Cargo.lock or pnpm-lock.yaml. Use the
# upgrade recipes above to actually apply changes.
deps-outdated:
    #!/usr/bin/env bash
    set -euo pipefail

    have() { command -v "$1" &>/dev/null; }

    echo "==> Rust workspace (cargo upgrade --dry-run)"
    if ! have cargo-upgrade; then
        echo "  cargo-upgrade unavailable — run 'just bootstrap' to install cargo-edit" >&2
    else
        {{ cargo }} upgrade --dry-run --incompatible 2>&1 | sed 's/^/  /'
    fi

    echo
    echo "==> Node workspace (pnpm outdated)"
    pnpm outdated -r --format list 2>&1 | sed 's/^/  /' || true

# ============================================================================
# Methodology self-hosting (tanren-repo specific)
#
# Dogfoods the methodology installer: renders embedded commands + the
# selected standards profile into the three agent targets and writes
# the MCP server registration into each agent's config. Adopters need
# not replicate these recipes — `tanren-cli install` is the single
# public-facing entry point; these are convenience wrappers over it
# for local development and CI drift-detection on the tanren repo
# itself.
# ============================================================================

# Render the command catalog and standards per `tanren.yml`.
# DISABLED until tanren-cli is rebuilt against the new architecture.
install-commands:
    @echo "tanren-cli not yet rebuilt — see CLAUDE.md."

# Preview installer writes without mutating files.
install-commands-dry-run:
    @echo "tanren-cli not yet rebuilt — see CLAUDE.md."

# Strict dry-run: fail if rendered artifacts drift from the plan.
# Re-wire into `just ci` once tanren-cli is rebuilt.
install-commands-check:
    @echo "tanren-cli not yet rebuilt — see CLAUDE.md."

# ============================================================================
# Build
# ============================================================================

# Build all workspace crates.
build:
    @CARGO_INCREMENTAL=0 {{ cargo }} build --workspace --locked --quiet

# Fast local guardrails: lock/dependency/format/static checks only, no tests.
check:
    #!/usr/bin/env bash
    set -euo pipefail

    now_ms() {
        perl -MTime::HiRes=time -e 'printf "%.0f\n", time * 1000'
    }

    fmt_duration() {
        perl -e 'printf "%.2fs", $ARGV[0] / 1000' "$1"
    }

    run_stage() {
        local name="$1"
        shift
        local start
        start="$(now_ms)"
        echo "==> ${name}"
        set +e
        "$@"
        local status="$?"
        set -e
        local elapsed="$(( $(now_ms) - start ))"
        if [[ "${status}" -eq 0 ]]; then
            echo "<== ${name} ok ($(fmt_duration "${elapsed}"))"
        else
            echo "<== ${name} failed ($(fmt_duration "${elapsed}"))"
        fi
        return "${status}"
    }

    total_start="$(now_ms)"
    run_stage "deps locked" just deps-locked-check
    run_stage "format" just fmt
    run_stage "workflow lint" just workflow-lint
    run_stage "docs" just docs-check
    run_stage "line budget" just check-lines
    run_stage "suppression guard" just check-suppression
    run_stage "dependency boundaries" just check-deps
    run_stage "rust test surface" just check-rust-test-surface
    run_stage "bdd tags" just check-bdd-tags
    run_stage "cargo check" bash -c 'CARGO_INCREMENTAL=0 {{ cargo }} check --workspace --all-targets --locked --quiet'
    run_stage "clippy" bash -c 'CARGO_INCREMENTAL=0 {{ cargo }} clippy --workspace --all-targets --locked --quiet -- -D warnings'
    total_elapsed="$(( $(now_ms) - total_start ))"
    echo "<== check total ($(fmt_duration "${total_elapsed}"))"

# ============================================================================
# Test
# ============================================================================

# Canonical behavior verification path. F-0001 ships zero feature files;
# every R-* slice from R-0001 onwards adds its `B-XXXX.feature` file under
# tests/bdd/features/. The harness machinery (cucumber-rs World, step
# registry) is exercised by Rust unit tests inside tanren-bdd — those are
# the only `#[test]` items in the workspace, enforced by
# `just check-rust-test-surface`.
tests:
    #!/usr/bin/env bash
    set -euo pipefail
    {{ cargo }} test -p tanren-bdd --locked --quiet
    {{ cargo }} run -q -p tanren-bdd --bin tanren-bdd-runner --locked

# Deep mutation verification path. Runs cargo-mutants against the BDD crate
# step-definition machinery. Reserved for nightly main-branch CI; not part
# of `just ci`. With zero scenarios shipped in F-0001 the report is
# necessarily empty — the assertion is that the pipeline runs without error.
mutation:
    #!/usr/bin/env bash
    set -euo pipefail
    {{ cargo }} mutants --package tanren-bdd --check
    echo "mutation: pipeline ran (no real scenarios yet — empty report is expected)"

# ============================================================================
# Lint & Format
# ============================================================================

# Run clippy with deny warnings.
lint:
    @CARGO_INCREMENTAL=0 {{ cargo }} clippy --workspace --all-targets --locked --quiet -- -D warnings

# Glob for Rust workspace TOML files. Crate-member globs (bin/*/Cargo.toml etc.)
# are added back when those crates are restored.
toml_globs := "Cargo.toml .cargo/*.toml rust-toolchain.toml clippy.toml taplo.toml deny.toml .rustfmt.toml lefthook.yml"

# Check formatting (Rust + TOML + Markdown).
fmt:
    @{{ cargo }} fmt --check
    @RUST_LOG=error taplo fmt --check {{ toml_globs }}
    @just markdown-fmt

# Auto-fix formatting (Rust + TOML + Markdown).
fmt-fix:
    @{{ cargo }} fmt
    @RUST_LOG=error taplo fmt {{ toml_globs }}
    @just markdown-fmt-fix

# Auto-fix everything that can be auto-fixed (formatting + clippy suggestions).
fix:
    @{{ cargo }} fmt
    @RUST_LOG=error taplo fmt {{ toml_globs }}
    @just markdown-fmt-fix
    @CARGO_INCREMENTAL=0 {{ cargo }} clippy --workspace --all-targets --fix --allow-dirty --allow-staged --quiet -- -D warnings

# ============================================================================
# Audit & Analysis
# ============================================================================

# Audit dependencies (licenses, advisories, bans, sources).
deny:
    #!/usr/bin/env bash
    set -euo pipefail
    output="$({{ cargo }} deny --log-level error check 2>&1)" && status=0 || status=$?
    if [[ "${status}" -ne 0 ]]; then
        echo "${output}"
        exit "${status}"
    fi

# Detect unused dependencies.
machete:
    #!/usr/bin/env bash
    set -euo pipefail
    output="$({{ cargo }} machete 2>&1)" && status=0 || status=$?
    if [[ "${status}" -ne 0 ]]; then
        echo "${output}"
        exit "${status}"
    fi

# ============================================================================
# Documentation
# ============================================================================

# Check Markdown formatting with rumdl.
markdown-fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    output="$(git ls-files '*.md' ':!.claude/**' ':!.codex/**' ':!.opencode/**' | xargs rumdl fmt --check --quiet 2>&1)" && status=0 || status=$?
    if [[ "${status}" -ne 0 ]]; then
        echo "${output}"
        exit "${status}"
    fi

# Auto-fix Markdown formatting with rumdl.
markdown-fmt-fix:
    #!/usr/bin/env bash
    set -euo pipefail
    git ls-files '*.md' ':!.claude/**' ':!.codex/**' ':!.opencode/**' | xargs rumdl fmt

# Lint Markdown with rumdl.
markdown-lint:
    #!/usr/bin/env bash
    set -euo pipefail
    output="$(git ls-files '*.md' ':!.claude/**' ':!.codex/**' ':!.opencode/**' | xargs rumdl check --quiet 2>&1)" && status=0 || status=$?
    if [[ "${status}" -ne 0 ]]; then
        echo "${output}"
        exit "${status}"
    fi

# Check local Markdown links and anchors with lychee.
markdown-links:
    #!/usr/bin/env bash
    set -euo pipefail
    output="$(git ls-files '*.md' ':!.claude/**' ':!.codex/**' ':!.opencode/**' | xargs lychee --offline --include-fragments --no-progress --quiet 2>&1)" && status=0 || status=$?
    if [[ "${status}" -ne 0 ]]; then
        echo "${output}"
        exit "${status}"
    fi

# Check Markdown lint, links, and anchors.
docs-check:
    @just markdown-lint
    @just markdown-links

# Lint GitHub Actions workflow syntax and expressions.
workflow-lint:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v actionlint &>/dev/null; then
        echo "FAIL: actionlint is unavailable. Run 'just bootstrap'." >&2
        exit 127
    fi
    actionlint

# Build documentation (warnings are errors).
doc:
    @RUSTDOCFLAGS="-D warnings" CARGO_INCREMENTAL=0 {{ cargo }} doc --workspace --no-deps --locked --quiet

# ============================================================================
# Quality Gates
# ============================================================================

# Enforce max file line count (500 lines per .rs file).
check-lines:
    #!/usr/bin/env bash
    set -euo pipefail
    failed=0
    while IFS= read -r -d '' file; do
        lines=$(wc -l < "$file")
        if [[ "$lines" -gt {{ max_lines }} ]]; then
            echo "FAIL: $file has $lines lines (max {{ max_lines }})"
            failed=1
        fi
    done < <(find crates/ bin/ -name '*.rs' -print0)
    if [[ "$failed" -eq 1 ]]; then exit 1; fi

# Enforce crate dependency layering (foundation crates must not depend on capability crates).
check-deps:
    #!/usr/bin/env bash
    set -euo pipefail

    foundation=(
        "tanren-domain"
        "tanren-contract"
        "tanren-policy"
        "tanren-observability"
    )
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
    for fnd in "${foundation[@]}"; do
        deps=$(echo "$metadata" | jq -r ".packages[] | select(.name == \"$fnd\") | .dependencies[] | select(.kind == null) | .name" 2>/dev/null || true)
        for cap in "${capability[@]}"; do
            if echo "$deps" | grep -qx "$cap"; then
                echo "FAIL: foundation crate '$fnd' depends on capability crate '$cap'"
                failed=1
            fi
        done
    done

    transport=("tanren-cli" "tanren-api" "tanren-mcp" "tanren-tui")
    forbidden_transport=("tanren-domain" "tanren-policy" "tanren-store" "tanren-planner" "tanren-scheduler" "tanren-orchestrator")
    for bin in "${transport[@]}"; do
        deps=$(echo "$metadata" | jq -r ".packages[] | select(.name == \"$bin\") | .dependencies[] | select(.kind == null) | .name" 2>/dev/null || true)
        for forbidden in "${forbidden_transport[@]}"; do
            if echo "$deps" | grep -qx "$forbidden"; then
                echo "FAIL: transport binary '$bin' depends directly on '$forbidden'"
                failed=1
            fi
        done
    done

    if grep -Eq '^[[:space:]]*pub mod entity;' crates/tanren-store/src/lib.rs; then
        echo "FAIL: crates/tanren-store/src/lib.rs exports 'pub mod entity;'"
        failed=1
    fi
    if grep -Eq '^[[:space:]]*pub mod (dispatch_projection|events|step_projection);' crates/tanren-store/src/entity/mod.rs; then
        echo "FAIL: crates/tanren-store/src/entity/mod.rs exposes row-shape modules publicly"
        failed=1
    fi

    # F-0002: tanren-mcp must serve over HTTP (axum-based stack), per
    # docs/architecture/subsystems/interfaces.md#mcp and
    # docs/architecture/technology.md (rejected-alternatives: stdio MCP).
    # The presence of axum in the dependency closure is the mechanical
    # signal that the binary is wired to the HTTP transport rather than
    # rmcp's stdio transport.
    mcp_deps=$(echo "$metadata" | jq -r '.packages[] | select(.name == "tanren-mcp") | .dependencies[] | select(.kind == null) | .name' 2>/dev/null || true)
    if ! echo "$mcp_deps" | grep -qx "axum"; then
        echo "FAIL: tanren-mcp must depend on axum (HTTP transport mandated by architecture)"
        failed=1
    fi

    if [[ "$failed" -eq 1 ]]; then
        echo "Dependency/boundary rule violations detected."
        exit 1
    fi

# Enforce BDD-only Rust test surface and retired gate names.
check-rust-test-surface:
    @{{ cargo }} run --quiet -p tanren-xtask -- check-rust-test-surface

# Enforce the F-0002 BDD `.feature` convention: filename↔@B-XXXX, closed
# tag allowlist, strict-equality interface coverage against
# docs/behaviors and docs/roadmap/dag.json. See
# docs/architecture/subsystems/behavior-proof.md (BDD Tagging And File
# Convention).
check-bdd-tags:
    @{{ cargo }} run --quiet -p tanren-xtask -- check-bdd-tags

# Verify active rustc/clippy match the pinned toolchain in rust-toolchain.toml.
check-rust-toolchain-sync:
    #!/usr/bin/env bash
    set -euo pipefail

    expected="$(awk -F'"' '/^channel[[:space:]]*=/{print $2; exit}' rust-toolchain.toml)"
    if [[ -z "${expected:-}" ]]; then
        echo "FAIL: rust-toolchain.toml missing pinned channel."
        exit 1
    fi

    rustc_version="$(rustc -V | awk '{print $2}')"
    if [[ "$rustc_version" != "$expected" ]]; then
        echo "FAIL: active rustc is ${rustc_version}, expected ${expected} from rust-toolchain.toml."
        exit 1
    fi

    rustc_hash="$(rustc -Vv | awk '$1 == "commit-hash:" {print $2}')"
    clippy_hash="$({{ cargo }} clippy --version | awk -F'[()]' '{print $2}' | awk '{print $1}' | awk '/^[0-9a-f]+$/ {print; exit}')"
    if [[ -z "${rustc_hash:-}" || -z "${clippy_hash:-}" ]]; then
        echo "FAIL: unable to parse rustc/clippy version metadata."
        exit 1
    fi

    rustc_short="${rustc_hash:0:10}"
    if [[ "$clippy_hash" != "$rustc_short" ]]; then
        echo "FAIL: clippy commit ${clippy_hash} does not match rustc commit ${rustc_short}."
        exit 1
    fi

# Prohibit inline lint suppression (#[allow/expect]).
check-suppression:
    #!/usr/bin/env bash
    set -euo pipefail
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

# ============================================================================
# Maintenance
# ============================================================================

# Remove build artifacts.
clean:
    {{ cargo }} clean

# ============================================================================
# CI
# ============================================================================

# Run full PR gate locally.
ci:
    #!/usr/bin/env bash
    set -euo pipefail

    now_ms() {
        perl -MTime::HiRes=time -e 'printf "%.0f\n", time * 1000'
    }

    fmt_duration() {
        perl -e 'printf "%.2fs", $ARGV[0] / 1000' "$1"
    }

    run_stage() {
        local name="$1"
        shift
        local start
        start="$(now_ms)"
        echo "==> ${name}"
        set +e
        "$@"
        local status="$?"
        set -e
        local elapsed="$(( $(now_ms) - start ))"
        if [[ "${status}" -eq 0 ]]; then
            echo "<== ${name} ok ($(fmt_duration "${elapsed}"))"
        else
            echo "<== ${name} failed ($(fmt_duration "${elapsed}"))"
        fi
        return "${status}"
    }

    run_stage_quiet() {
        local name="$1"
        shift
        local start
        start="$(now_ms)"
        local output
        echo "==> ${name}"
        set +e
        output="$("$@" 2>&1)"
        local status="$?"
        set -e
        local elapsed="$(( $(now_ms) - start ))"
        if [[ "${status}" -eq 0 ]]; then
            echo "<== ${name} ok ($(fmt_duration "${elapsed}"))"
        else
            echo "${output}"
            echo "<== ${name} failed ($(fmt_duration "${elapsed}"))"
        fi
        return "${status}"
    }

    total_start="$(now_ms)"
    run_stage "check" just check
    run_stage_quiet "tests" just tests
    run_stage "deny" just deny
    run_stage "doc" just doc
    run_stage "machete" just machete
    run_stage "web install" just web-install
    run_stage_quiet "web build" just web-build
    run_stage_quiet "web lint" just web-lint
    run_stage_quiet "web typecheck" just web-typecheck
    run_stage_quiet "web format" just web-format-check
    # Disabled until tanren-cli is rebuilt:
    # run_stage "install drift" just install-commands-check
    total_elapsed="$(( $(now_ms) - total_start ))"
    echo "<== ci total ($(fmt_duration "${total_elapsed}"))"
    echo "==> ci passed"

# ============================================================================
# Web frontend (apps/web/)
# ============================================================================

# Install pnpm workspace dependencies. Lockfile must be up to date.
web-install:
    pnpm install --frozen-lockfile

# Build the web frontend (Next.js + Turbopack).
web-build:
    pnpm --filter @tanren/web build

# Lint the web frontend (oxlint).
web-lint:
    pnpm --filter @tanren/web lint

# Typecheck the web frontend (tsgo from @typescript/native-preview).
web-typecheck:
    pnpm --filter @tanren/web typecheck

# Format the web frontend (auto-fix).
web-format:
    pnpm --filter @tanren/web format

# Format the web frontend (check only — used in CI gate).
web-format-check:
    pnpm --filter @tanren/web format:check

# Run web frontend unit tests.
web-test:
    pnpm --filter @tanren/web test
