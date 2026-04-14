"""Detect direct store-bypass patterns in Rust interface layers.

Interface layers must not construct store persistence params/events or call store
mutation methods directly. These actions belong in orchestrator/store crates.

Usage:
    uv run python scripts/check_store_bypass.py

Exit code 0 if clean, 1 if violations found.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

INTERFACE_GLOBS = [
    "bin/tanren-cli/src/**/*.rs",
    "bin/tanren-api/src/**/*.rs",
    "bin/tanren-mcp/src/**/*.rs",
    "bin/tanren-tui/src/**/*.rs",
    "crates/tanren-app-services/src/**/*.rs",
]

FORBIDDEN_STORE_INTERNAL_IMPORTS = [
    re.compile(r"\btanren_store::entity::"),
    re.compile(r"\btanren_store::migration::"),
]

FORBIDDEN_STORE_CONSTRUCTORS = [
    "CreateDispatchParams",
    "CreateDispatchWithInitialStepParams",
    "UpdateDispatchStatusParams",
    "CancelDispatchParams",
    "CancelPendingStepsParams",
    "EnqueueStepParams",
    "AckParams",
    "AckAndEnqueueParams",
    "NackParams",
    "EventEnvelope",
]

FORBIDDEN_TRANSPORT_CALLS = [
    "create_dispatch_projection",
    "create_dispatch_with_initial_step",
    "update_dispatch_status",
    "cancel_dispatch",
    "cancel_pending_steps",
    "enqueue_step",
    "ack",
    "ack_and_enqueue",
    "nack",
    "append",
    "append_batch",
]


def iter_interface_files() -> list[Path]:
    """Return all Rust interface source files that should be audited."""
    files: set[Path] = set()
    for pattern in INTERFACE_GLOBS:
        files.update(Path().glob(pattern))
    return sorted(path for path in files if path.is_file())


def strip_line_comments(line: str) -> str:
    """Return `line` without trailing `//` comment content."""
    comment_start = line.find("//")
    if comment_start == -1:
        return line
    return line[:comment_start]


def is_transport_binary_file(path: Path) -> bool:
    """Return whether `path` points at a transport binary source tree."""
    parts = path.parts
    return len(parts) > 1 and parts[0] == "bin"


def check_file(path: Path) -> list[str]:
    """Return store-bypass violations found in one Rust source file."""
    violations: list[str] = []
    lines = path.read_text().splitlines()

    for lineno, raw_line in enumerate(lines, start=1):
        line = strip_line_comments(raw_line)
        if not line.strip():
            continue

        violations.extend(
            (
                f"  {path}:{lineno} — imports store internals `{pattern.pattern}`. "
                "Use public store/app-service contracts only."
            )
            for pattern in FORBIDDEN_STORE_INTERNAL_IMPORTS
            if pattern.search(line)
        )

        violations.extend(
            (f"  {path}:{lineno} — constructs `{constructor}` outside orchestrator/store.")
            for constructor in FORBIDDEN_STORE_CONSTRUCTORS
            if re.search(rf"\b{re.escape(constructor)}\s*\{{", line)
        )

        if is_transport_binary_file(path):
            violations.extend(
                (
                    f"  {path}:{lineno} — direct `{method_name}(...)` call in "
                    "transport binary. Route through tanren-app-services."
                )
                for method_name in FORBIDDEN_TRANSPORT_CALLS
                if re.search(rf"\.\s*{re.escape(method_name)}\s*\(", line)
            )

    return violations


def main() -> int:
    """Run the store-bypass audit and return an exit status.

    Returns:
        Exit status code (0 on success, 1 when violations are found).
    """
    files = iter_interface_files()
    all_violations: list[str] = []
    for path in files:
        all_violations.extend(check_file(path))

    if all_violations:
        print("ERROR: Rust store-bypass violations detected:\n")
        for violation in all_violations:
            print(violation)
        print(f"\n{len(all_violations)} violation(s) found across {len(files)} files.")
        return 1

    print(f"Rust store-bypass check passed ({len(files)} files scanned).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
