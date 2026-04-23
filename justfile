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
        "cargo-mutants:cargo-mutants"
        "cargo-upgrade:cargo-edit"
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
    {{ cargo }} fetch --locked
    {{ cargo }} build --workspace --locked

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

# ============================================================================
# Methodology self-hosting (tanren-repo specific)
#
# Dogfoods the methodology installer: renders `commands/` + the
# bundled baseline standards into the three agent targets and writes
# the MCP server registration into each agent's config. Adopters need
# not replicate these recipes — `tanren-cli install` is the single
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
    @{{ cargo }} build --workspace --locked --quiet

# Run per-task quality gate checks (fast, deterministic, no test matrix)
check:
    @just deps-locked-check
    @just fmt
    @just check-lines
    @just check-suppression
    @just check-deps
    @{{ cargo }} check --workspace --all-targets --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --locked --quiet
    @{{ cargo }} clippy --workspace --all-targets --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --locked --quiet -- -D warnings

# ============================================================================
# Test
# ============================================================================

# Run all tests via nextest (pass extra args after --)
test *args:
    @{{ cargo }} nextest run --workspace --no-tests=pass --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --locked {{ nextest_quiet_flags }} {{ args }}

# Generate code coverage report (lcov)
coverage:
    @{{ cargo }} llvm-cov nextest --workspace --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --locked --lcov --output-path lcov.info --no-tests=pass
    @echo "Coverage report: lcov.info"

# ============================================================================
# Lint & Format
# ============================================================================

# Run clippy with deny warnings
lint:
    @{{ cargo }} clippy --workspace --all-targets --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --locked --quiet -- -D warnings

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
    @RUSTDOCFLAGS="-D warnings" {{ cargo }} doc --workspace --no-deps --locked --quiet

# ============================================================================
# Quality Gates
# ============================================================================

# Phase 0 staged scenario gate scaffold (compatibility mode).
check-phase0-scenario-stage:
    #!/usr/bin/env bash
    set -euo pipefail

    bdd_source="docs/rewrite/PHASE0_PROOF_BDD.md"
    scenario_count=0
    if [[ -f "$bdd_source" ]]; then
        scenario_count="$(grep -Ec '^### Scenario [0-9]+\.[0-9]+:' "$bdd_source" || true)"
        echo "Phase 0 scenario scaffold: found ${scenario_count} scenario headings in ${bdd_source}."
    else
        echo "Phase 0 scenario scaffold: ${bdd_source} not found (non-blocking in scaffold mode)."
    fi

    feature_count=0
    if [[ -d tests/bdd/phase0 ]]; then
        feature_count="$(find tests/bdd/phase0 -type f -name '*.feature' | wc -l | tr -d '[:space:]')"
    fi
    echo "Phase 0 scenario scaffold: detected ${feature_count} feature file(s) under tests/bdd/phase0."

# Phase 0 strict scenario gate (enforced in final flow).
check-phase0-scenario-gate:
    #!/usr/bin/env bash
    set -euo pipefail

    bdd_source="docs/rewrite/PHASE0_PROOF_BDD.md"
    if [[ ! -f "$bdd_source" ]]; then
        echo "FAIL: missing Phase 0 scenario source at ${bdd_source}."
        exit 1
    fi

    scenario_count="$(grep -Ec '^### Scenario [0-9]+\.[0-9]+:' "$bdd_source" || true)"
    if [[ "${scenario_count}" -lt 1 ]]; then
        echo "FAIL: expected at least one scenario heading in ${bdd_source}."
        exit 1
    fi
    echo "Phase 0 strict scenario gate: found ${scenario_count} scenario heading(s) in ${bdd_source}."

    if [[ ! -d tests/bdd/phase0 ]]; then
        echo "FAIL: missing tests/bdd/phase0 directory."
        exit 1
    fi
    feature_count="$(find tests/bdd/phase0 -type f -name '*.feature' | wc -l | tr -d '[:space:]')"
    if [[ "${feature_count}" -lt 9 ]]; then
        echo "FAIL: expected at least 9 feature files under tests/bdd/phase0; found ${feature_count}."
        exit 1
    fi
    echo "Phase 0 strict scenario gate: detected ${feature_count} feature file(s) under tests/bdd/phase0."

# Run a cucumber-rs smoke scenario for the Phase 0 BDD harness scaffold.
check-phase0-bdd-smoke:
    #!/usr/bin/env bash
    set -euo pipefail

    feature_file="tests/bdd/phase0/smoke.feature"
    if [[ ! -f "$feature_file" ]]; then
        echo "FAIL: missing Phase 0 smoke feature at ${feature_file}."
        exit 1
    fi

    {{ cargo }} run --quiet -p tanren-bdd-phase0 --locked

    tag_line="$(grep -E '^@' "$feature_file" | head -n 1 || true)"
    if [[ -z "$tag_line" ]]; then
        echo "FAIL: no tag line found in ${feature_file}."
        exit 1
    fi
    echo "Phase 0 BDD smoke tag evidence: ${tag_line}"

# Run Wave A cucumber-rs scenarios (Features 1-3) for Phase 0.
check-phase0-bdd-wave-a:
    #!/usr/bin/env bash
    set -euo pipefail

    feature_files=(
        "tests/bdd/phase0/feature-1-typed-control-plane-state.feature"
        "tests/bdd/phase0/feature-2-event-history.feature"
        "tests/bdd/phase0/feature-3-contract-derived-interface.feature"
    )

    for feature_file in "${feature_files[@]}"; do
        if [[ ! -f "$feature_file" ]]; then
            echo "FAIL: missing Phase 0 Wave A feature at ${feature_file}."
            exit 1
        fi
        TANREN_BDD_PHASE0_FEATURE_PATH="$feature_file" \
            {{ cargo }} run --quiet -p tanren-bdd-phase0 --locked

        tag_line="$(grep -E '^[[:space:]]*@.*@BEH-P0-(101|102|201|202|203|301|302)' "$feature_file" | head -n 1 || true)"
        if [[ -z "$tag_line" ]]; then
            echo "FAIL: no Wave A behavior tag line found in ${feature_file}."
            exit 1
        fi
        echo "Phase 0 Wave A BDD tag evidence (${feature_file}): ${tag_line}"
    done

# Run Wave B cucumber-rs scenarios (Features 4-6) for Phase 0.
check-phase0-bdd-wave-b:
    #!/usr/bin/env bash
    set -euo pipefail

    feature_files=(
        "tests/bdd/phase0/feature-4-methodology-boundary.feature"
        "tests/bdd/phase0/feature-5-task-completion-guards.feature"
        "tests/bdd/phase0/feature-6-tool-surface-contract.feature"
    )

    for feature_file in "${feature_files[@]}"; do
        if [[ ! -f "$feature_file" ]]; then
            echo "FAIL: missing Phase 0 Wave B feature at ${feature_file}."
            exit 1
        fi
        TANREN_BDD_PHASE0_FEATURE_PATH="$feature_file" \
            {{ cargo }} run --quiet -p tanren-bdd-phase0 --locked

        tag_line="$(grep -E '^[[:space:]]*@.*@BEH-P0-(401|402|501|502|601|602|603)' "$feature_file" | head -n 1 || true)"
        if [[ -z "$tag_line" ]]; then
            echo "FAIL: no Wave B behavior tag line found in ${feature_file}."
            exit 1
        fi
        echo "Phase 0 Wave B BDD tag evidence (${feature_file}): ${tag_line}"
    done

# Run Wave C cucumber-rs scenarios (Features 7-8) for Phase 0.
check-phase0-bdd-wave-c:
    #!/usr/bin/env bash
    set -euo pipefail

    feature_files=(
        "tests/bdd/phase0/feature-7-installer-determinism.feature"
        "tests/bdd/phase0/feature-8-manual-methodology-walkthrough.feature"
    )

    for feature_file in "${feature_files[@]}"; do
        if [[ ! -f "$feature_file" ]]; then
            echo "FAIL: missing Phase 0 Wave C feature at ${feature_file}."
            exit 1
        fi
        TANREN_BDD_PHASE0_FEATURE_PATH="$feature_file" \
            {{ cargo }} run --quiet -p tanren-bdd-phase0 --locked

        tag_line="$(grep -E '^[[:space:]]*@.*@BEH-P0-(701|702|703|801)' "$feature_file" | head -n 1 || true)"
        if [[ -z "$tag_line" ]]; then
            echo "FAIL: no Wave C behavior tag line found in ${feature_file}."
            exit 1
        fi
        echo "Phase 0 Wave C BDD tag evidence (${feature_file}): ${tag_line}"
    done

# Phase 0 staged mutation gate (compatibility mode).
check-phase0-mutation-stage:
    #!/usr/bin/env bash
    set -euo pipefail

    ./scripts/proof/phase0/run_mutation_stage.sh

# Phase 0 strict mutation gate (enforced in final flow).
check-phase0-mutation-gate:
    #!/usr/bin/env bash
    set -euo pipefail

    PHASE0_MUTATION_ENFORCE=1 ./scripts/proof/phase0/run_mutation_stage.sh

# Phase 0 staged coverage classification gate (compatibility mode).
check-phase0-coverage-stage:
    #!/usr/bin/env bash
    set -euo pipefail

    ./scripts/proof/phase0/run_coverage_stage.sh

# Phase 0 strict coverage classification gate (enforced in final flow).
check-phase0-coverage-gate:
    #!/usr/bin/env bash
    set -euo pipefail

    PHASE0_COVERAGE_ENFORCE=1 ./scripts/proof/phase0/run_coverage_stage.sh

# Run all staged Phase 0 behavior gate scaffolds.
check-phase0-stage-gates:
    @just check-phase0-scenario-stage
    @just check-phase0-bdd-smoke
    @just check-phase0-bdd-wave-a
    @just check-phase0-bdd-wave-b
    @just check-phase0-bdd-wave-c
    @just check-phase0-mutation-stage
    @just check-phase0-coverage-stage

# Run all strict Phase 0 behavior gates (mandatory in final flow).
check-phase0-gates:
    @just check-phase0-scenario-gate
    @just check-phase0-bdd-smoke
    @just check-phase0-bdd-wave-a
    @just check-phase0-bdd-wave-b
    @just check-phase0-bdd-wave-c
    @just check-phase0-mutation-gate
    @just check-phase0-coverage-gate

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
    clippy_hash="$(
        cargo clippy --version \
            | awk -F'[()]' '{print $2}' \
            | awk '{print $1}' \
            | awk '/^[0-9a-f]+$/ {print; exit}'
    )"
    if [[ -z "${rustc_hash:-}" || -z "${clippy_hash:-}" ]]; then
        echo "FAIL: unable to parse rustc/clippy version metadata."
        exit 1
    fi

    rustc_short="${rustc_hash:0:10}"
    if [[ "$clippy_hash" != "$rustc_short" ]]; then
        echo "FAIL: clippy commit ${clippy_hash} does not match rustc commit ${rustc_short}."
        exit 1
    fi
    echo "Rust toolchain sync check passed (${expected}, ${rustc_short})."

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

# Run redaction benchmark scenarios used by the perf regression gate.
bench-redaction:
    @{{ cargo }} bench -p tanren-runtime --bench redaction -- --noplot

# Enforce benchmark thresholds for redaction scenarios.
check-redaction-perf:
    @just bench-redaction
    @UV_CACHE_DIR="${UV_CACHE_DIR:-$PWD/.uv-cache}" uv run python scripts/check_redaction_perf.py

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
    @echo "==> Lockfile guard"
    @just deps-locked-check
    @echo "==> Toolchain sync"
    @just check-rust-toolchain-sync
    @echo "==> Clippy"
    @RUSTFLAGS="-D warnings" {{ cargo }} clippy --workspace --all-targets --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --locked --quiet -- -D warnings
    @echo "==> Build tanren-mcp for parity tests"
    @RUSTFLAGS="-D warnings" {{ cargo }} build -p tanren-mcp --locked --quiet
    @echo "==> Workspace tests"
    @target_dir="${CARGO_TARGET_DIR:-target}"; if [[ "$target_dir" = /* ]]; then tanren_mcp_bin="${target_dir}/debug/tanren-mcp"; else tanren_mcp_bin="$PWD/${target_dir}/debug/tanren-mcp"; fi; if [[ ! -x "$tanren_mcp_bin" ]]; then echo "FAIL: tanren-mcp test binary missing at ${tanren_mcp_bin}."; exit 1; fi; RUSTFLAGS="-D warnings" TANREN_MCP_BIN="$tanren_mcp_bin" {{ cargo }} nextest run --workspace --features tanren-store/test-hooks,tanren-orchestrator/test-hooks --profile ci --locked --no-tests=pass {{ nextest_quiet_flags }}
    @echo "==> Postgres integration"
    @./scripts/run_postgres_integration.sh

# Run full CI check locally.
ci:
    @echo "==> Task gate checks"
    @just check
    @echo "==> CI parity guard"
    @just check-ci-parity
    @echo "==> Phase 0 strict behavior gates"
    @just check-phase0-gates
    @echo "==> Dependency audit"
    @just deny
    @echo "==> Docs"
    @just doc
    @echo "==> Unused dependency audit"
    @just machete
    @echo "==> Strict Rust CI"
    @just ci-rust-strict
    @echo "==> Redaction perf regression gate"
    @just check-redaction-perf
    @echo "==> Installer drift guard"
    @just install-commands-check
    @echo "==> All CI checks passed!"
