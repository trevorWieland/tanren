"""Detect direct store protocol calls from interface layers.

Interface code (routers, MCP tool handlers, CLI commands) must not call
store methods directly — all mutations go through the service layer or
the dispatch_orchestrator in tanren_core.

Usage:
    uv run python scripts/check_store_bypass.py

Exit code 0 if clean, 1 if violations found.
"""

from __future__ import annotations

import ast
import sys
from pathlib import Path

# Store protocol methods that should never be called from interface code.
FORBIDDEN_METHODS = frozenset({
    "create_dispatch_projection",
    "update_dispatch_status",
    "cancel_pending_steps",
    "enqueue_step",
    "create_api_key",
    "revoke_api_key",
    "set_grace_replacement",
    "create_user_projection",
    "deactivate_user",
})

# Files to scan (interface layers only).
INTERFACE_FILES = [
    *Path("services/tanren-api/src/tanren_api/routers").glob("*.py"),
    Path("services/tanren-api/src/tanren_api/mcp_server.py"),
    *Path("services/tanren-cli/src/tanren_cli").glob("*_cli.py"),
]


def check_file(path: Path) -> list[str]:
    """Return list of violation messages for a single file."""
    source = path.read_text()
    try:
        tree = ast.parse(source, filename=str(path))
    except SyntaxError:
        return []

    violations: list[str] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        if isinstance(func, ast.Attribute) and func.attr in FORBIDDEN_METHODS:
            violations.append(
                f"  {path}:{node.lineno} — calls store method '{func.attr}'. "
                "Route through service/orchestrator."
            )
    return violations


def main() -> int:
    """Run the checker.

    Returns:
        Exit code: 0 if clean, 1 if violations found.
    """
    all_violations: list[str] = []
    for path in INTERFACE_FILES:
        if not path.exists():
            continue
        all_violations.extend(check_file(path))

    if all_violations:
        print("ERROR: Store protocol bypass detected in interface code:\n")
        for v in all_violations:
            print(v)
        print(f"\n{len(all_violations)} violation(s) found.")
        return 1

    print(f"Store bypass check passed ({len(INTERFACE_FILES)} files scanned).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
