"""Detect business logic leaking into interface layers.

Interface code (routers, MCP tool handlers, CLI commands) should be thin:
parse input, check auth, delegate to service, format output.  Step payload
construction, raw Dispatch building, state guard patterns, and manual step
sequencing belong in the service/orchestrator layer.

Usage:
    uv run python scripts/check_thin_interfaces.py

Exit code 0 if clean, 1 if violations found.
"""

from __future__ import annotations

import ast
import sys
from pathlib import Path

# Payload classes that should never be instantiated in interface code.
FORBIDDEN_CONSTRUCTORS = frozenset({
    "ProvisionStepPayload",
    "ExecuteStepPayload",
    "TeardownStepPayload",
    "DryRunStepPayload",
})

# Enum members that indicate state-guard logic in interface code.
GUARD_PATTERN_NAMES = frozenset({
    "StepStatus",
    "StepType",
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
        # Check for step payload construction
        if isinstance(node, ast.Call):
            func = node.func
            name = None
            if isinstance(func, ast.Name):
                name = func.id
            elif isinstance(func, ast.Attribute):
                name = func.attr
            if name in FORBIDDEN_CONSTRUCTORS:
                violations.append(
                    f"  {path}:{node.lineno} — constructs '{name}' in interface code. "
                    "Move to service/orchestrator."
                )

        # Check for step_type/step_status comparisons inside any() calls —
        # this is the state guard pattern (e.g., any(s.step_type == StepType.EXECUTE ...)).
        # Simple comparisons like `step.status == StepStatus.FAILED` are
        # result-reading for output and are allowed.
        if isinstance(node, ast.Call):
            func = node.func
            if isinstance(func, ast.Name) and func.id == "any":
                # Walk the any() arguments for guard patterns
                for child in ast.walk(node):
                    if isinstance(child, ast.Compare):
                        for comparator in [child.left, *child.comparators]:
                            if (
                                isinstance(comparator, ast.Attribute)
                                and isinstance(comparator.value, ast.Name)
                                and comparator.value.id in GUARD_PATTERN_NAMES
                            ):
                                violations.append(
                                    f"  {path}:{node.lineno} — state guard pattern "
                                    f"({comparator.value.id}.{comparator.attr}) inside any() "
                                    "in interface code. Move guards to service/orchestrator."
                                )
                                break

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
        print("ERROR: Business logic detected in interface code:\n")
        for v in all_violations:
            print(v)
        print(f"\n{len(all_violations)} violation(s) found.")
        return 1

    print(f"Thin interface check passed ({len(INTERFACE_FILES)} files scanned).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
