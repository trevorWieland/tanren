#!/usr/bin/env python3
"""Bump version across all workspace pyproject.toml files.

Keeps every package in the monorepo at the same version. Prints the new
version to stdout so CI can capture it.

Usage:
    python3 scripts/bump_version.py              # 0.2.0 -> 0.2.1
    python3 scripts/bump_version.py --part minor # 0.2.0 -> 0.3.0
    python3 scripts/bump_version.py --part major # 0.2.0 -> 1.0.0
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

PYPROJECT_FILES = [
    "pyproject.toml",
    "packages/tanren-core/pyproject.toml",
    "services/tanren-api/pyproject.toml",
    "services/tanren-cli/pyproject.toml",
    "services/tanren-daemon/pyproject.toml",
]

VERSION_RE = re.compile(r'^(version\s*=\s*")(\d+\.\d+\.\d+)(")', re.MULTILINE)


def find_version(text: str) -> str | None:
    """Return the first version = "x.y.z" value, or None."""
    m = VERSION_RE.search(text)
    return m.group(2) if m else None


def bump(version: str, part: str) -> str:
    """Increment the requested part of a semver string.

    Returns:
        The bumped version string.
    """
    major, minor, patch = (int(x) for x in version.split("."))
    if part == "major":
        return f"{major + 1}.0.0"
    if part == "minor":
        return f"{major}.{minor + 1}.0"
    return f"{major}.{minor}.{patch + 1}"


def main() -> None:
    """Bump version in all workspace pyproject.toml files."""
    parser = argparse.ArgumentParser(description="Bump workspace version")
    parser.add_argument(
        "--part",
        choices=["patch", "minor", "major"],
        default="patch",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    paths = [root / f for f in PYPROJECT_FILES]

    # Read all files and verify consistent versions
    contents: dict[Path, str] = {}
    versions: set[str] = set()
    for p in paths:
        if not p.exists():
            print(f"ERROR: file not found: {p}", file=sys.stderr)
            sys.exit(1)
        text = p.read_text()
        v = find_version(text)
        if v is None:
            print(f"ERROR: no version found in {p}", file=sys.stderr)
            sys.exit(1)
        contents[p] = text
        versions.add(v)

    if len(versions) != 1:
        print(f"ERROR: inconsistent versions: {versions}", file=sys.stderr)
        sys.exit(1)

    old = versions.pop()
    new = bump(old, args.part)

    for p in paths:
        updated = VERSION_RE.sub(rf"\g<1>{new}\3", contents[p], count=1)
        p.write_text(updated)

    print(new)


if __name__ == "__main__":
    main()
