"""Detect architecture leakage in Rust transport interface code.

Transport binaries (CLI/API/MCP/TUI) must stay thin: parse input, delegate to
app-services, and format output. They must not depend on domain/store/
orchestrator/policy internals directly.

Usage:
    uv run python scripts/check_thin_interfaces.py

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
]

FORBIDDEN_PATH_PATTERNS = [
    re.compile(r"\btanren_domain::"),
    re.compile(r"\btanren_store::"),
    re.compile(r"\btanren_orchestrator::"),
    re.compile(r"\btanren_policy::"),
]


def iter_interface_files() -> list[Path]:
    """Return all transport binary Rust files to audit."""
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


def check_file(path: Path) -> list[str]:
    """Return thin-interface violations found in one file."""
    violations: list[str] = []
    lines = path.read_text().splitlines()
    for lineno, raw_line in enumerate(lines, start=1):
        line = strip_line_comments(raw_line)
        if not line.strip():
            continue
        violations.extend(
            (
                f"  {path}:{lineno} — forbidden direct dependency in transport binary: "
                f"`{pattern.pattern}`. Route through tanren-app-services/contract."
            )
            for pattern in FORBIDDEN_PATH_PATTERNS
            if pattern.search(line)
        )
    return violations


def main() -> int:
    """Run the thin-interface audit and return an exit status.

    Returns:
        Exit status code (0 on success, 1 when violations are found).
    """
    files = iter_interface_files()
    all_violations: list[str] = []
    for path in files:
        all_violations.extend(check_file(path))

    if all_violations:
        print("ERROR: Rust transport thin-interface violations detected:\n")
        for violation in all_violations:
            print(violation)
        print(f"\n{len(all_violations)} violation(s) found across {len(files)} files.")
        return 1

    print(f"Rust thin-interface check passed ({len(files)} files scanned).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
