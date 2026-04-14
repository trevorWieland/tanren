"""Assert local `just ci` and GitHub workflow rust checks stay aligned.

This is a lightweight drift guard: both files must include the strict
Rust command paths we consider parity-critical.
"""

from __future__ import annotations

from pathlib import Path

WORKFLOW_PATH = Path(".github/workflows/rust-ci.yml")
JUSTFILE_PATH = Path("justfile")

WORKFLOW_REQUIRED = [
    'RUSTFLAGS: "-D warnings"',
    ("cargo clippy --workspace --all-targets --features tanren-store/test-hooks -- -D warnings"),
    (
        "cargo nextest run --workspace --features tanren-store/test-hooks "
        "--profile ci --no-tests=pass"
    ),
    (
        "cargo nextest run -p tanren-store --features "
        "tanren-store/test-hooks,tanren-store/postgres-integration --no-tests=pass"
    ),
]

JUST_REQUIRED = [
    (
        'RUSTFLAGS="-D warnings" {{ cargo }} clippy --workspace --all-targets '
        "--features tanren-store/test-hooks -- -D warnings"
    ),
    (
        'RUSTFLAGS="-D warnings" {{ cargo }} nextest run --workspace '
        "--features tanren-store/test-hooks --profile ci --no-tests=pass"
    ),
    (
        'RUSTFLAGS="-D warnings" {{ cargo }} nextest run -p tanren-store '
        "--features tanren-store/test-hooks,tanren-store/postgres-integration "
        "--no-tests=pass"
    ),
    "check-ci-parity:",
]


def missing_snippets(content: str, snippets: list[str]) -> list[str]:
    """Return snippets that are absent from `content`."""
    return [snippet for snippet in snippets if snippet not in content]


def main() -> int:
    """Validate parity-critical command snippets in CI and justfile.

    Returns:
        Exit status code (0 on success, 1 when required snippets are missing).
    """
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    justfile = JUSTFILE_PATH.read_text(encoding="utf-8")

    missing_workflow = missing_snippets(workflow, WORKFLOW_REQUIRED)
    missing_just = missing_snippets(justfile, JUST_REQUIRED)

    if missing_workflow or missing_just:
        print("ERROR: CI parity drift detected.")
        if missing_workflow:
            print("\nMissing from workflow:")
            for snippet in missing_workflow:
                print(f"  - {snippet}")
        if missing_just:
            print("\nMissing from justfile:")
            for snippet in missing_just:
                print(f"  - {snippet}")
        return 1

    print("CI parity check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
