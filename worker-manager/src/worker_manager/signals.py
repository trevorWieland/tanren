"""Signal extraction and outcome mapping per PROTOCOL.md Section 3."""

import json
import re
from pathlib import Path

from worker_manager.schemas import (
    Finding,
    FindingsOutput,
    InvestigationReport,
    Outcome,
    Phase,
)


def extract_signal(
    phase: Phase,
    command_name: str,
    spec_folder_path: Path,
    stdout: str,
) -> str | None:
    """Extract agent signal from status file, audit.md, or stdout fallback.

    For audit-spec: reads audit.md first line for status: pass|fail|unknown.
    For others: reads .agent-status for {command}-status: {signal}, fallback to stdout grep.
    """
    if phase == Phase.GATE or phase == Phase.SETUP or phase == Phase.CLEANUP:
        return None

    # audit-spec special case: status comes from audit.md
    if phase == Phase.AUDIT_SPEC:
        return _extract_audit_spec_signal(spec_folder_path)

    # Primary: read .agent-status file
    status_file = spec_folder_path / ".agent-status"
    if status_file.exists():
        content = status_file.read_text()
        pattern = rf"{re.escape(command_name)}-status:\s*(\w[\w-]*)"
        match = re.search(pattern, content)
        if match:
            return match.group(1)

    # Fallback: grep stdout
    if stdout:
        pattern = rf"{re.escape(command_name)}-status:\s*(\w[\w-]*)"
        matches = re.findall(pattern, stdout)
        if matches:
            return matches[-1]  # Last match, like tail -1 in bash

    return None


def _extract_audit_spec_signal(spec_folder_path: Path) -> str | None:
    """Extract signal from audit.md first line: status: pass|fail|unknown."""
    audit_path = spec_folder_path / "audit.md"
    if not audit_path.exists():
        return None
    try:
        first_line = audit_path.read_text().split("\n", 1)[0].strip()
        match = re.match(r"status:\s*(pass|fail|unknown)", first_line, re.IGNORECASE)
        if match:
            status = match.group(1).lower()
            if status == "unknown":
                return None  # Treated as error by outcome mapping
            return status
    except Exception:
        pass
    return None


def map_outcome(
    phase: Phase,
    signal: str | None,
    exit_code: int,
    timed_out: bool,
) -> tuple[Outcome, str | None]:
    """Map raw signal, exit code, and timeout to (outcome, signal) per PROTOCOL.md Section 3.

    Returns the mapped (outcome, signal) tuple.
    """
    # Timeout always wins
    if timed_out:
        return (Outcome.TIMEOUT, None)

    # Gate phases: simple exit code mapping
    if phase == Phase.GATE:
        if exit_code == 0:
            return (Outcome.SUCCESS, None)
        return (Outcome.FAIL, None)

    # Setup/cleanup phases: exit code mapping
    if phase in (Phase.SETUP, Phase.CLEANUP):
        if exit_code == 0:
            return (Outcome.SUCCESS, None)
        return (Outcome.ERROR, None)

    # Agent phases: signal-based mapping
    if signal is not None:
        match signal:
            case "complete":
                return (Outcome.SUCCESS, "complete")
            case "pass":
                return (Outcome.SUCCESS, "pass")
            case "all-done":
                return (Outcome.SUCCESS, "all-done")
            case "fail":
                return (Outcome.FAIL, "fail")
            case "blocked":
                return (Outcome.BLOCKED, "blocked")
            case "error":
                return (Outcome.ERROR, "error")
            case _:
                # Unrecognized signal — treat as success with warning
                return (Outcome.SUCCESS, signal)

    # No signal: fall back to exit code
    if exit_code == 0:
        return (Outcome.SUCCESS, None)
    return (Outcome.ERROR, None)


# --- Structured findings parsing ---


def parse_audit_findings(spec_folder_path: Path) -> FindingsOutput | None:
    """Parse {spec_folder}/audit-findings.json."""
    return _parse_findings_file(spec_folder_path / "audit-findings.json")


def parse_demo_findings(spec_folder_path: Path) -> FindingsOutput | None:
    """Parse {spec_folder}/demo-findings.json."""
    return _parse_findings_file(spec_folder_path / "demo-findings.json")


def parse_audit_spec_findings(spec_folder_path: Path) -> list[Finding]:
    """Extract structured findings from audit.md between markers."""
    audit_path = spec_folder_path / "audit.md"
    if not audit_path.exists():
        return []
    content = audit_path.read_text()
    match = re.search(
        r"<!--\s*structured-findings-start\s*-->(.*?)<!--\s*structured-findings-end\s*-->",
        content,
        re.DOTALL,
    )
    if not match:
        return []
    try:
        raw = json.loads(match.group(1).strip())
        return [Finding.model_validate(f) for f in raw]
    except Exception:
        return []


def parse_investigation_report(spec_folder_path: Path) -> InvestigationReport | None:
    """Parse {spec_folder}/investigation-report.json."""
    path = spec_folder_path / "investigation-report.json"
    if not path.exists():
        return None
    try:
        return InvestigationReport.model_validate_json(path.read_text())
    except Exception:
        return None


def _parse_findings_file(path: Path) -> FindingsOutput | None:
    """Parse a findings JSON file with best-effort fallback."""
    if not path.exists():
        return None
    try:
        return FindingsOutput.model_validate_json(path.read_text())
    except Exception:
        pass
    # Best-effort: strip markdown code fences and retry
    try:
        content = path.read_text().strip()
        content = re.sub(r"^```\w*\n?", "", content)
        content = re.sub(r"\n?```$", "", content)
        return FindingsOutput.model_validate_json(content)
    except Exception:
        return None
