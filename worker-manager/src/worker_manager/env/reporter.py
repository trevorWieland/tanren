"""Terminal-formatted and JSON validation reports."""

import json
import os
import sys

from worker_manager.env.validator import EnvReport, VarResult, VarStatus


def supports_color() -> bool:
    """Check if terminal supports color output."""
    if os.environ.get("NO_COLOR"):
        return False
    return hasattr(sys.stderr, "isatty") and sys.stderr.isatty()


def _icon(status: VarStatus, color: bool) -> str:
    """Return status icon."""
    if color:
        match status:
            case VarStatus.PASS:
                return "\033[32m\u2713\033[0m"  # green checkmark
            case VarStatus.MISSING | VarStatus.EMPTY:
                return "\033[31m\u2717\033[0m"  # red X
            case VarStatus.PATTERN_MISMATCH:
                return "\033[31m\u2717\033[0m"  # red X
            case VarStatus.DEFAULTED:
                return "\033[33m\u25cb\033[0m"  # yellow circle
    else:
        match status:
            case VarStatus.PASS:
                return "[OK]"
            case VarStatus.MISSING | VarStatus.EMPTY | VarStatus.PATTERN_MISMATCH:
                return "[FAIL]"
            case VarStatus.DEFAULTED:
                return "[DEFAULT]"
    return "?"


def _format_var(var: VarResult, color: bool, verbose: bool) -> str | None:
    """Format a single var result line."""
    if var.status == VarStatus.PASS and not verbose:
        return None

    icon = _icon(var.status, color)
    parts = [f"  {icon} {var.key}"]

    if var.source:
        parts.append(f"(from {var.source})")

    if var.message:
        parts.append(f"— {var.message}")
    elif var.description and var.status != VarStatus.PASS:
        parts.append(f"— {var.description}")

    if var.hint and var.status in (VarStatus.MISSING, VarStatus.EMPTY):
        parts.append(f"\n      hint: {var.hint}")

    return " ".join(parts)


def format_report(
    report: EnvReport,
    project_name: str = "",
    config_path: str = "tanren.yml",
    verbose: bool = False,
) -> str:
    """Box-formatted terminal output for env validation."""
    color = supports_color()
    lines: list[str] = []

    header = "Environment Validation"
    if project_name:
        header += f" — {project_name}"
    lines.append(header)
    lines.append(f"Config: {config_path}")
    lines.append("")

    # Required section
    if report.required_results:
        lines.append("Required:")
        for var in report.required_results:
            line = _format_var(var, color, verbose)
            if line is not None:
                lines.append(line)
        # Show pass count when not verbose
        if not verbose:
            pass_count = sum(1 for v in report.required_results if v.status == VarStatus.PASS)
            if pass_count:
                lines.append(f"  ... {pass_count} passed (use --verbose to show)")
        lines.append("")

    # Optional section
    if report.optional_results:
        lines.append("Optional:")
        for var in report.optional_results:
            line = _format_var(var, color, verbose)
            if line is not None:
                lines.append(line)
        if not verbose:
            pass_count = sum(1 for v in report.optional_results if v.status == VarStatus.PASS)
            if pass_count:
                lines.append(f"  ... {pass_count} passed (use --verbose to show)")
        lines.append("")

    # Warnings
    for w in report.warnings:
        lines.append(f"  WARNING: {w}")

    # Summary
    if report.passed:
        status = "\033[32mPASSED\033[0m" if color else "PASSED"
    else:
        status = "\033[31mFAILED\033[0m" if color else "FAILED"
    lines.append(f"Result: {status}")

    return "\n".join(lines)


def format_report_json(report: EnvReport) -> str:
    """JSON output for programmatic use."""
    data = {
        "passed": report.passed,
        "required": [
            {
                "key": v.key,
                "status": v.status.value,
                "description": v.description,
                "hint": v.hint,
                "source": v.source,
                "message": v.message,
            }
            for v in report.required_results
        ],
        "optional": [
            {
                "key": v.key,
                "status": v.status.value,
                "description": v.description,
                "source": v.source,
                "message": v.message,
            }
            for v in report.optional_results
        ],
        "warnings": report.warnings,
    }
    return json.dumps(data, indent=2)
