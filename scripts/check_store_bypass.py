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

FORBIDDEN_APP_SERVICES_CALLS = [
    "create_dispatch_projection",
    "create_dispatch_with_initial_step",
    "update_dispatch_status",
    "cancel_pending_steps",
    "enqueue_step",
    "ack",
    "ack_and_enqueue",
    "nack",
    "append",
    "append_batch",
]

FORBIDDEN_APP_SERVICES_CANCEL_CALL = "cancel_dispatch"

CANCEL_DISPATCH_ALLOWED_RECEIVER_MARKERS = ("orchestrator",)


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


def strip_line_comments_from_text(text: str) -> str:
    """Return `text` with `//` comments removed while preserving line count."""
    return "\n".join(strip_line_comments(line) for line in text.splitlines())


def is_transport_binary_file(path: Path) -> bool:
    """Return whether `path` points at a transport binary source tree."""
    path_text = path.as_posix()
    if path_text.startswith("bin/"):
        return True
    return any(
        marker in path_text
        for marker in (
            "/bin/tanren-cli/src/",
            "/bin/tanren-api/src/",
            "/bin/tanren-mcp/src/",
            "/bin/tanren-tui/src/",
        )
    )


def is_app_services_file(path: Path) -> bool:
    """Return whether `path` points at the app-services source tree."""
    path_text = path.as_posix()
    return path_text.startswith("crates/tanren-app-services/src/") or (
        "/crates/tanren-app-services/src/" in path_text
    )


def line_number_for_offset(text: str, offset: int) -> int:
    """Convert a string offset to a 1-based line number."""
    return text.count("\n", 0, offset) + 1


def extract_receiver_expression(text: str, call_dot_offset: int) -> str:
    """Extract a best-effort receiver expression before a method call dot."""
    index = call_dot_offset - 1
    while index >= 0 and text[index].isspace():
        index -= 1
    receiver_end = index + 1
    while index >= 0 and text[index] not in ";\n{}":
        index -= 1
    receiver_start = index + 1
    return text[receiver_start:receiver_end].strip()


def find_method_calls(text: str, method_name: str) -> list[tuple[int, str]]:
    """Return `(line_number, receiver_expression)` for method call matches."""
    method_pattern = re.compile(rf"\.\s*{re.escape(method_name)}\s*\(")
    return [
        (line_number_for_offset(text, match.start()), extract_receiver_expression(text, match.start()))
        for match in method_pattern.finditer(text)
    ]


def find_constructor_occurrences(text: str, constructor_name: str) -> list[int]:
    """Return line numbers where a constructor literal is instantiated."""
    constructor_pattern = re.compile(rf"\b{re.escape(constructor_name)}\s*\{{")
    return [line_number_for_offset(text, match.start()) for match in constructor_pattern.finditer(text)]


def check_file(path: Path) -> list[str]:
    """Return store-bypass violations found in one Rust source file."""
    violations: list[str] = []
    seen: set[str] = set()
    raw_text = path.read_text(encoding="utf-8")
    text = strip_line_comments_from_text(raw_text)
    lines = text.splitlines()

    def add_violation(message: str) -> None:
        if message in seen:
            return
        seen.add(message)
        violations.append(message)

    for lineno, raw_line in enumerate(lines, start=1):
        if not raw_line.strip():
            continue

        for pattern in FORBIDDEN_STORE_INTERNAL_IMPORTS:
            if pattern.search(raw_line):
                add_violation(
                    f"  {path}:{lineno} — imports store internals `{pattern.pattern}`. "
                    "Use public store/app-service contracts only."
                )

    for constructor in FORBIDDEN_STORE_CONSTRUCTORS:
        for lineno in find_constructor_occurrences(text, constructor):
            add_violation(f"  {path}:{lineno} — constructs `{constructor}` outside orchestrator/store.")

    if is_transport_binary_file(path):
        for method_name in FORBIDDEN_TRANSPORT_CALLS:
            for lineno, _ in find_method_calls(text, method_name):
                add_violation(
                    f"  {path}:{lineno} — direct `{method_name}(...)` call in "
                    "transport binary. Route through tanren-app-services."
                )

    if is_app_services_file(path):
        for method_name in FORBIDDEN_APP_SERVICES_CALLS:
            for lineno, _ in find_method_calls(text, method_name):
                add_violation(
                    f"  {path}:{lineno} — direct `{method_name}(...)` call in "
                    "app-services. Route writes through tanren-orchestrator."
                )

        for lineno, receiver in find_method_calls(text, FORBIDDEN_APP_SERVICES_CANCEL_CALL):
            receiver_lower = receiver.lower()
            if any(marker in receiver_lower for marker in CANCEL_DISPATCH_ALLOWED_RECEIVER_MARKERS):
                continue
            add_violation(
                f"  {path}:{lineno} — direct `cancel_dispatch(...)` store-path call in "
                "app-services. Route cancellation through tanren-orchestrator."
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
