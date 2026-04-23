#!/usr/bin/env python3
# ruff: noqa: DOC201, E501
"""Render a machine-consumable Phase 0 coverage classification artifact."""

from __future__ import annotations

import argparse
import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

WAVE_SOURCE_MAP = {
    "A": "crates/tanren-bdd-phase0/src/main.rs",
    "B": "crates/tanren-bdd-phase0/src/wave_b_steps.rs",
    "C": "crates/tanren-bdd-phase0/src/wave_c_steps.rs",
}


def parse_args() -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--traceability", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--coverage-summary", type=Path, required=True)
    parser.add_argument("--feature-executions", type=Path, required=True)
    parser.add_argument("--status", required=True)
    parser.add_argument("--exit-code", type=int, required=True)
    parser.add_argument("--version", default="")
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    """Load JSON from `path`; return empty object when missing/invalid."""
    if not path.exists():
        return {}
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    if isinstance(raw, dict):
        return raw
    return {}


def normalize_path(path_str: str, repo_root: Path) -> str:
    """Normalize paths to repo-relative POSIX format where possible."""
    candidate = Path(path_str)
    if candidate.is_absolute():
        try:
            return candidate.relative_to(repo_root).as_posix()
        except ValueError:
            return candidate.as_posix()
    return candidate.as_posix()


def load_coverage_summary(
    summary_path: Path, repo_root: Path
) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    """Load per-file and total summary metrics from llvm-cov JSON."""
    payload = load_json(summary_path)
    data_rows = payload.get("data")
    if not isinstance(data_rows, list) or not data_rows:
        return {}, {}

    head = data_rows[0]
    if not isinstance(head, dict):
        return {}, {}

    files = head.get("files")
    totals = head.get("totals")
    if not isinstance(files, list):
        files = []
    if not isinstance(totals, dict):
        totals = {}

    by_file: dict[str, dict[str, Any]] = {}
    for row in files:
        if not isinstance(row, dict):
            continue
        filename = row.get("filename")
        summary = row.get("summary")
        if not isinstance(filename, str) or not filename:
            continue
        if not isinstance(summary, dict):
            continue

        lines = summary.get("lines", {})
        functions = summary.get("functions", {})
        regions = summary.get("regions", {})
        if not isinstance(lines, dict):
            lines = {}
        if not isinstance(functions, dict):
            functions = {}
        if not isinstance(regions, dict):
            regions = {}

        relpath = normalize_path(filename, repo_root)
        by_file[relpath] = {
            "path": relpath,
            "lines": {
                "count": int(lines.get("count", 0)),
                "covered": int(lines.get("covered", 0)),
                "percent": float(lines.get("percent", 0.0)),
            },
            "functions": {
                "count": int(functions.get("count", 0)),
                "covered": int(functions.get("covered", 0)),
                "percent": float(functions.get("percent", 0.0)),
            },
            "regions": {
                "count": int(regions.get("count", 0)),
                "covered": int(regions.get("covered", 0)),
                "percent": float(regions.get("percent", 0.0)),
            },
        }

    return by_file, totals


def load_feature_executions(path: Path) -> dict[str, dict[str, Any]]:
    """Load NDJSON feature execution rows keyed by feature path."""
    rows: dict[str, dict[str, Any]] = {}
    if not path.exists():
        return rows

    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(row, dict):
            continue
        feature_file = row.get("feature_file")
        if not isinstance(feature_file, str) or not feature_file:
            continue
        rows[feature_file] = {
            "status": row.get("status", "unknown"),
            "exit_code": int(row.get("exit_code", 0)),
        }
    return rows


def load_behavior_inventory(traceability_path: Path) -> list[dict[str, str]]:
    """Extract normalized behavior inventory rows from the traceability artifact."""
    payload = load_json(traceability_path)
    inventory = payload.get("behavior_inventory")
    if not isinstance(inventory, list):
        return []

    rows: list[dict[str, str]] = []
    for row in inventory:
        if not isinstance(row, dict):
            continue
        behavior_id = row.get("behavior_id")
        scenario_id = row.get("scenario_id")
        wave = row.get("wave")
        obligations = row.get("obligations")
        if not isinstance(behavior_id, str) or not behavior_id:
            continue
        if not isinstance(scenario_id, str):
            scenario_id = ""
        if not isinstance(wave, str):
            wave = ""
        planned_feature_file = ""
        if isinstance(obligations, dict):
            positive = obligations.get("positive")
            if isinstance(positive, dict):
                candidate = positive.get("planned_feature_file")
                if isinstance(candidate, str):
                    planned_feature_file = candidate

        rows.append({
            "behavior_id": behavior_id,
            "scenario_id": scenario_id,
            "wave": wave,
            "planned_feature_file": planned_feature_file,
        })
    return rows


def classify_behavior_coverage(
    inventory: list[dict[str, str]],
    feature_executions: dict[str, dict[str, Any]],
    coverage_by_file: dict[str, dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Classify behaviors as covered or missing using feature + source evidence."""
    covered: list[dict[str, Any]] = []
    missing: list[dict[str, Any]] = []

    for row in inventory:
        behavior_id = row["behavior_id"]
        wave = row["wave"]
        feature_path = row["planned_feature_file"]
        source_path = WAVE_SOURCE_MAP.get(wave)
        feature_exec = feature_executions.get(
            feature_path, {"status": "not_executed", "exit_code": 0}
        )
        source_cov = coverage_by_file.get(source_path, {}) if source_path else {}

        feature_status = str(feature_exec.get("status", "not_executed"))
        feature_exit_code = int(feature_exec.get("exit_code", 0))
        source_lines = source_cov.get("lines", {}) if isinstance(source_cov, dict) else {}
        source_lines_covered = (
            int(source_lines.get("covered", 0)) if isinstance(source_lines, dict) else 0
        )
        source_line_percent = (
            float(source_lines.get("percent", 0.0)) if isinstance(source_lines, dict) else 0.0
        )

        entry = {
            "behavior_id": behavior_id,
            "scenario_id": row["scenario_id"],
            "wave": wave,
            "planned_feature_file": feature_path,
            "feature_status": feature_status,
            "feature_exit_code": feature_exit_code,
            "source_path": source_path,
            "source_line_percent": source_line_percent,
        }

        if feature_status != "passed":
            entry["classification"] = "missing_behavior"
            entry["reason"] = "planned_feature_not_passing"
            missing.append(entry)
            continue

        if not source_path:
            entry["classification"] = "missing_behavior"
            entry["reason"] = "wave_has_no_source_mapping"
            missing.append(entry)
            continue

        if not source_cov:
            entry["classification"] = "missing_behavior"
            entry["reason"] = "source_missing_from_coverage_report"
            missing.append(entry)
            continue

        if source_lines_covered == 0:
            entry["classification"] = "missing_behavior"
            entry["reason"] = "source_has_zero_covered_lines"
            missing.append(entry)
            continue

        entry["classification"] = "covered_behavior"
        entry["reason"] = "planned_feature_passed_and_wave_source_covered"
        covered.append(entry)

    return covered, missing


def classify_non_behavior_code(
    coverage_by_file: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    """Classify covered files that are not behavior-owner source files."""
    behavior_sources = set(WAVE_SOURCE_MAP.values())
    rows: list[dict[str, Any]] = []

    for path, summary in sorted(coverage_by_file.items()):
        if path in behavior_sources:
            continue
        lines = summary.get("lines", {})
        lines_covered = int(lines.get("covered", 0)) if isinstance(lines, dict) else 0
        line_percent = float(lines.get("percent", 0.0)) if isinstance(lines, dict) else 0.0

        if lines_covered == 0:
            bucket = "dead_or_unexercised_support_code"
            rationale = "file is unmapped to BEH inventory and has zero covered lines"
        else:
            bucket = "support_code"
            rationale = "file is unmapped to BEH inventory but exercised during coverage run"

        rows.append({
            "path": path,
            "classification": bucket,
            "line_percent": line_percent,
            "rationale": rationale,
        })
    return rows


def coverage_files_payload(coverage_by_file: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    """Serialize per-file coverage summary for report output."""
    return [coverage_by_file[path] for path in sorted(coverage_by_file)]


def main() -> int:
    """Generate and write `classification.json` for staged coverage evidence."""
    args = parse_args()
    args.run_dir.mkdir(parents=True, exist_ok=True)

    repo_root = Path.cwd()
    coverage_by_file, coverage_totals = load_coverage_summary(args.coverage_summary, repo_root)
    feature_executions = load_feature_executions(args.feature_executions)
    behavior_inventory = load_behavior_inventory(args.traceability)

    covered_behaviors, missing_behaviors = classify_behavior_coverage(
        behavior_inventory, feature_executions, coverage_by_file
    )
    non_behavior_code = classify_non_behavior_code(coverage_by_file)

    report = {
        "schema_version": "1.0.0",
        "artifact": "phase0_coverage_classification",
        "generated_at": datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "stage": "staged_non_blocking",
        "run": {
            "status": args.status,
            "exit_code": args.exit_code,
            "cargo_llvm_cov_version": args.version,
            "package": "tanren-bdd-phase0",
            "bin": "tanren-bdd-phase0",
            "non_blocking": True,
        },
        "behavior_inventory": {
            "source": str(args.traceability),
            "total_count": len(behavior_inventory),
            "covered_count": len(covered_behaviors),
            "missing_count": len(missing_behaviors),
            "covered": covered_behaviors,
            "missing": missing_behaviors,
        },
        "classification": {
            "missing_behavior": missing_behaviors,
            "non_behavior_code": non_behavior_code,
        },
        "coverage_summary": {
            "totals": coverage_totals,
            "files": coverage_files_payload(coverage_by_file),
            "behavior_owner_sources": sorted(WAVE_SOURCE_MAP.values()),
        },
        "feature_executions": [
            {
                "feature_file": feature_file,
                "status": row["status"],
                "exit_code": row["exit_code"],
            }
            for feature_file, row in sorted(feature_executions.items())
        ],
        "policy": {
            "behavior_gap_triage": [
                "Any behavior listed under classification.missing_behavior requires scenario coverage remediation before final enforcement.",
                "Feature execution failures take precedence over source-level percentages when classifying missing behavior.",
            ],
            "non_behavior_code_triage": [
                "Unmapped files are tracked separately from behavior gaps.",
                "Zero-covered unmapped files should be reviewed as dead/support candidates, not behavior-missing scenarios.",
            ],
        },
        "artifacts": {
            "command": str(args.run_dir / "command.txt"),
            "feature_executions": str(args.feature_executions),
            "coverage_summary_json": str(args.coverage_summary),
            "lcov": str(args.run_dir / "lcov.info"),
            "coverage_run_stdout": str(args.run_dir / "coverage-run.stdout.log"),
            "coverage_run_stderr": str(args.run_dir / "coverage-run.stderr.log"),
            "coverage_lcov_stdout": str(args.run_dir / "coverage-lcov.stdout.log"),
            "coverage_lcov_stderr": str(args.run_dir / "coverage-lcov.stderr.log"),
        },
    }

    out_path = args.run_dir / "classification.json"
    out_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    print(
        "Phase 0 coverage classification artifact generated: "
        f"{out_path} (covered={len(covered_behaviors)}, missing={len(missing_behaviors)})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
