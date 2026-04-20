"""Assert local `just ci` and GitHub workflow rust checks stay aligned.

This is a lightweight drift guard: both files must include the strict
Rust command paths we consider parity-critical.
"""

from __future__ import annotations

from pathlib import Path

WORKFLOW_PATH = Path(".github/workflows/rust-ci.yml")
JUSTFILE_PATH = Path("justfile")
WRAPPER_PATH = Path("scripts/run_postgres_integration.sh")

WORKFLOW_REQUIRED = [
    'RUSTFLAGS: "-D warnings"',
    'RUST_TOOLCHAIN: "1.94.1"',
    "toolchain: ${{ env.RUST_TOOLCHAIN }}",
    (
        "cargo clippy --workspace --all-targets "
        "--features tanren-store/test-hooks,tanren-orchestrator/test-hooks "
        "--locked -- -D warnings"
    ),
    (
        "cargo nextest run --workspace "
        "--features tanren-store/test-hooks,tanren-orchestrator/test-hooks "
        "--profile ci --locked --no-tests=pass"
    ),
    "Build tanren-mcp for workspace parity tests",
    (
        "cargo nextest run -j1 -p tanren-store --features "
        "tanren-store/test-hooks,tanren-store/postgres-integration --locked --no-tests=pass"
    ),
    (
        "cargo nextest run -j1 -p tanren-cli "
        "--features tanren-cli/postgres-integration --locked --no-tests=pass"
    ),
    "cargo build -p tanren-mcp --locked --quiet",
    "Build tanren-mcp for CLI parity integration tests",
    "TANREN_MCP_BIN: ${{ github.workspace }}/target/debug/tanren-mcp",
    "Check pinned Rust toolchain sync",
]

JUST_REQUIRED = [
    "deps-locked-check:",
    "@just deps-locked-check",
    "check-rust-toolchain-sync:",
    "@just check-rust-toolchain-sync",
    (
        'RUSTFLAGS="-D warnings" {{ cargo }} clippy --workspace --all-targets '
        "--features tanren-store/test-hooks,tanren-orchestrator/test-hooks --locked --quiet -- -D warnings"
    ),
    '{{ cargo }} build -p tanren-mcp --locked --quiet',
    'RUSTFLAGS="-D warnings" TANREN_MCP_BIN="$tanren_mcp_bin" {{ cargo }} nextest run --workspace',
    'TANREN_MCP_BIN="$tanren_mcp_bin" {{ cargo }} nextest run --workspace',
    "./scripts/run_postgres_integration.sh",
    "check-ci-parity:",
]

WRAPPER_REQUIRED = [
    'RUSTFLAGS="-D warnings" "${CARGO}" build -p tanren-mcp --locked --quiet',
    'candidate="${PWD}/${target_dir}/debug/tanren-mcp"',
    'export TANREN_MCP_BIN="${candidate}"',
    (
        '"${CARGO}" nextest run \\\n'
        "            -j1 \\\n"
        "            -p tanren-store \\\n"
        "            --features tanren-store/test-hooks,tanren-store/postgres-integration \\\n"
        "            --locked \\\n"
        '            "${NEXTEST_QUIET_FLAGS[@]}" \\\n'
        "            --no-tests=pass"
    ),
    (
        '"${CARGO}" nextest run \\\n'
        "            -j1 \\\n"
        "            -p tanren-cli \\\n"
        "            --features tanren-cli/postgres-integration \\\n"
        "            --locked \\\n"
        '            "${NEXTEST_QUIET_FLAGS[@]}" \\\n'
        "            --no-tests=pass"
    ),
]


def missing_snippets(content: str, snippets: list[str]) -> list[str]:
    """Return snippets that are absent from `content`."""
    return [snippet for snippet in snippets if snippet not in content]


def main() -> int:
    """Validate parity-critical command snippets in CI, justfile, and wrapper.

    Returns:
        Exit status code (0 on success, 1 when required snippets are missing).
    """
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    justfile = JUSTFILE_PATH.read_text(encoding="utf-8")
    wrapper = WRAPPER_PATH.read_text(encoding="utf-8")

    missing_workflow = missing_snippets(workflow, WORKFLOW_REQUIRED)
    missing_just = missing_snippets(justfile, JUST_REQUIRED)
    missing_wrapper = missing_snippets(wrapper, WRAPPER_REQUIRED)

    if missing_workflow or missing_just or missing_wrapper:
        print("ERROR: CI parity drift detected.")
        if missing_workflow:
            print("\nMissing from workflow:")
            for snippet in missing_workflow:
                print(f"  - {snippet}")
        if missing_just:
            print("\nMissing from justfile:")
            for snippet in missing_just:
                print(f"  - {snippet}")
        if missing_wrapper:
            print("\nMissing from scripts/run_postgres_integration.sh:")
            for snippet in missing_wrapper:
                print(f"  - {snippet}")
        return 1

    print("CI parity check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
