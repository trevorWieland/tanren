#!/usr/bin/env python3
# ruff: noqa: DOC201, E501, TC003
"""Render a machine-consumable Phase 0 mutation triage artifact."""

from __future__ import annotations

import argparse
import json
import re
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

SURVIVOR_OUTCOME_FILES = {
    "missed": "missed.txt",
    "timeout": "timeout.txt",
}
SUMMARY_OUTCOME_FILES = {
    "caught": "caught.txt",
    "missed": "missed.txt",
    "timeout": "timeout.txt",
    "unviable": "unviable.txt",
}
SOURCE_LINE_PATTERN = re.compile(r"(?P<path>[^\s:]+\.rs):(?P<line>\d+)")


@dataclass(frozen=True)
class SurvivorRecord:
    """One survivor entry prepared for triage."""

    outcome: str
    mutant: str
    source_path: str | None
    source_line: int | None
    behavior_ids: list[str]
    linkage_mode: str


def parse_args() -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--traceability", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--mutants-out", type=Path, required=True)
    parser.add_argument("--status", required=True)
    parser.add_argument("--exit-code", type=int, required=True)
    parser.add_argument("--version", default="")
    parser.add_argument("--shard", required=True)
    return parser.parse_args()


def read_lines(path: Path) -> list[str]:
    """Return non-empty, stripped lines from a text file if it exists."""
    if not path.exists():
        return []
    return [line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def resolve_mutants_out_dir(mutants_out: Path) -> Path:
    """Resolve the actual cargo-mutants output directory.

    cargo-mutants may emit either:
    - <run>/mutants.out/*
    - <run>/mutants.out/mutants.out/*
    """
    candidates = (mutants_out, mutants_out / "mutants.out")
    for candidate in candidates:
        if (candidate / "outcomes.json").exists():
            return candidate
        if any((candidate / name).exists() for name in SUMMARY_OUTCOME_FILES.values()):
            return candidate
    return mutants_out


def load_outcomes_payload(mutants_out: Path) -> dict[str, Any] | None:
    """Load cargo-mutants outcomes.json when present."""
    outcomes_path = mutants_out / "outcomes.json"
    if not outcomes_path.exists():
        return None
    payload = json.loads(outcomes_path.read_text(encoding="utf-8"))
    return payload if isinstance(payload, dict) else None


def parse_count(value: Any) -> int:
    """Parse an integer count field safely."""
    if isinstance(value, bool):
        return 0
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return int(value)
    if isinstance(value, str):
        try:
            return int(value)
        except ValueError:
            return 0
    return 0


def mutant_label_from_outcome(row: dict[str, Any]) -> str | None:
    """Extract a stable mutant label from one outcomes.json row."""
    scenario = row.get("scenario")
    if isinstance(scenario, dict):
        mutant = scenario.get("Mutant")
        if isinstance(mutant, dict):
            name = mutant.get("name")
            if isinstance(name, str) and name.strip():
                return name.strip()
    fallback = row.get("mutant")
    if isinstance(fallback, str) and fallback.strip():
        return fallback.strip()
    return None


def load_wave_behavior_ids(traceability_path: Path) -> tuple[dict[str, list[str]], list[str]]:
    """Load behavior IDs grouped by wave from the traceability artifact."""
    payload = json.loads(traceability_path.read_text(encoding="utf-8"))
    inventory = payload.get("behavior_inventory", [])

    by_wave: dict[str, list[str]] = {}
    all_ids: list[str] = []
    for row in inventory:
        if not isinstance(row, dict):
            continue
        behavior_id = row.get("behavior_id")
        wave = row.get("wave")
        if not isinstance(behavior_id, str) or not behavior_id:
            continue
        all_ids.append(behavior_id)
        if isinstance(wave, str) and wave:
            by_wave.setdefault(wave, []).append(behavior_id)

    for wave, ids in by_wave.items():
        by_wave[wave] = sorted(set(ids))

    return by_wave, sorted(set(all_ids))


def map_behavior_ids(
    source_path: str | None, wave_ids: dict[str, list[str]], all_ids: list[str]
) -> tuple[list[str], str]:
    """Map a mutant source path to one or more BEH IDs."""
    if source_path is None:
        return [], "unmapped"

    normalized = source_path.replace("\\", "/")
    if normalized.endswith("crates/tanren-bdd-phase0/src/wave_b_steps.rs"):
        return wave_ids.get("B", []), "wave_map"
    if normalized.endswith("crates/tanren-bdd-phase0/src/wave_c_steps.rs"):
        return wave_ids.get("C", []), "wave_map"
    if normalized.endswith("crates/tanren-bdd-phase0/src/main.rs"):
        return wave_ids.get("A", all_ids), "wave_map"
    if "crates/tanren-bdd-phase0/src/" in normalized:
        return all_ids, "crate_coarse"
    return [], "unmapped"


def parse_source_ref(mutant_label: str) -> tuple[str | None, int | None]:
    """Extract `<file>.rs:<line>` when present in a mutant label."""
    match = SOURCE_LINE_PATTERN.search(mutant_label)
    if not match:
        return None, None

    source_line: int | None = None
    try:
        source_line = int(match.group("line"))
    except ValueError:
        source_line = None
    return match.group("path"), source_line


def build_survivors(
    mutants_out: Path, wave_ids: dict[str, list[str]], all_ids: list[str]
) -> list[SurvivorRecord]:
    """Load survivor outcomes and attach BEH-ID linkage."""
    resolved_out = resolve_mutants_out_dir(mutants_out)
    dedupe: set[tuple[str, str]] = set()
    rows: list[SurvivorRecord] = []

    outcomes_payload = load_outcomes_payload(resolved_out)
    if outcomes_payload is not None:
        outcomes = outcomes_payload.get("outcomes")
        if isinstance(outcomes, list):
            for row in outcomes:
                if not isinstance(row, dict):
                    continue
                summary = row.get("summary")
                if not isinstance(summary, str):
                    continue
                outcome = summary.strip().lower()
                if outcome not in SURVIVOR_OUTCOME_FILES:
                    continue
                mutant = mutant_label_from_outcome(row)
                if mutant is None:
                    continue
                key = (outcome, mutant)
                if key in dedupe:
                    continue
                dedupe.add(key)
                source_path, source_line = parse_source_ref(mutant)
                behavior_ids, linkage_mode = map_behavior_ids(source_path, wave_ids, all_ids)
                rows.append(
                    SurvivorRecord(
                        outcome=outcome,
                        mutant=mutant,
                        source_path=source_path,
                        source_line=source_line,
                        behavior_ids=behavior_ids,
                        linkage_mode=linkage_mode,
                    )
                )

    # Fallback/compat path for txt summaries (or to fill missing labels).
    for outcome, filename in SURVIVOR_OUTCOME_FILES.items():
        for mutant in read_lines(resolved_out / filename):
            key = (outcome, mutant)
            if key in dedupe:
                continue
            dedupe.add(key)
            source_path, source_line = parse_source_ref(mutant)
            behavior_ids, linkage_mode = map_behavior_ids(source_path, wave_ids, all_ids)
            rows.append(
                SurvivorRecord(
                    outcome=outcome,
                    mutant=mutant,
                    source_path=source_path,
                    source_line=source_line,
                    behavior_ids=behavior_ids,
                    linkage_mode=linkage_mode,
                )
            )

    return rows


def count_outcomes(mutants_out: Path) -> dict[str, int]:
    """Count outcome lines from cargo-mutants summary files."""
    resolved_out = resolve_mutants_out_dir(mutants_out)
    outcomes_payload = load_outcomes_payload(resolved_out)

    if outcomes_payload is not None:
        counts = {
            "caught_count": parse_count(outcomes_payload.get("caught")),
            "missed_count": parse_count(outcomes_payload.get("missed")),
            "timeout_count": parse_count(outcomes_payload.get("timeout")),
            "unviable_count": parse_count(outcomes_payload.get("unviable")),
        }
        total_mutants = parse_count(outcomes_payload.get("total_mutants"))
        if total_mutants > 0:
            counts["tested_count"] = total_mutants
        else:
            counts["tested_count"] = (
                counts["caught_count"]
                + counts["missed_count"]
                + counts["timeout_count"]
                + counts["unviable_count"]
            )
        return counts

    counts_fallback: dict[str, int] = {}
    for outcome, filename in SUMMARY_OUTCOME_FILES.items():
        counts_fallback[f"{outcome}_count"] = len(read_lines(resolved_out / filename))
    counts_fallback["tested_count"] = (
        counts_fallback["caught_count"]
        + counts_fallback["missed_count"]
        + counts_fallback["timeout_count"]
        + counts_fallback["unviable_count"]
    )
    return counts_fallback


def serialize_survivors(rows: Iterable[SurvivorRecord]) -> list[dict[str, object]]:
    """Serialize survivors as plain dictionaries for JSON output."""
    return [
        {
            "outcome": row.outcome,
            "mutant": row.mutant,
            "source_path": row.source_path,
            "source_line": row.source_line,
            "behavior_ids": row.behavior_ids,
            "linkage_mode": row.linkage_mode,
            "triage_required": row.outcome in {"missed", "timeout"},
            "recommended_action": (
                "tighten_or_add_behavior_scenario"
                if row.outcome == "missed"
                else "investigate_timeout_and_record_behavior_link"
            ),
        }
        for row in rows
    ]


def main() -> int:
    """Generate and write `triage.json` for staged mutation-gate evidence."""
    args = parse_args()
    args.run_dir.mkdir(parents=True, exist_ok=True)

    wave_ids, all_ids = load_wave_behavior_ids(args.traceability)
    resolved_mutants_out = resolve_mutants_out_dir(args.mutants_out)
    survivors = build_survivors(resolved_mutants_out, wave_ids, all_ids)
    counts = count_outcomes(resolved_mutants_out)

    report = {
        "schema_version": "1.0.0",
        "artifact": "phase0_mutation_triage",
        "generated_at": datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "stage": "staged_non_blocking",
        "run": {
            "status": args.status,
            "exit_code": args.exit_code,
            "cargo_mutants_version": args.version,
            "shard": args.shard,
            "package": "tanren-bdd-phase0",
            "test_package": "tanren-bdd-phase0",
            "files": [
                "crates/tanren-bdd-phase0/src/main.rs",
                "crates/tanren-bdd-phase0/src/wave_b_steps.rs",
                "crates/tanren-bdd-phase0/src/wave_c_steps.rs",
            ],
            "baseline": "skip",
            "non_blocking": True,
        },
        "outcomes": counts,
        "survivors": serialize_survivors(survivors),
        "policy": {
            "survivor_triage": [
                "Every missed/timeout mutant must be linked to at least one BEH-* id.",
                "Resolve by tightening an existing scenario, adding a falsification scenario, or marking equivalent with explicit justification.",
                "Unmapped survivors must be escalated before final enforcement.",
            ],
            "linkage_sources": {
                "traceability_inventory": str(args.traceability),
                "wave_mappings": {
                    "A": wave_ids.get("A", []),
                    "B": wave_ids.get("B", []),
                    "C": wave_ids.get("C", []),
                },
            },
        },
        "artifacts": {
            "command": str(args.run_dir / "command.txt"),
            "stdout": str(args.run_dir / "cargo-mutants.stdout.log"),
            "stderr": str(args.run_dir / "cargo-mutants.stderr.log"),
            "requested_mutants_out": str(args.mutants_out),
            "mutants_out": str(resolved_mutants_out),
            "mutants_json": str(resolved_mutants_out / "mutants.json"),
            "outcomes_json": str(resolved_mutants_out / "outcomes.json"),
            "caught_txt": str(resolved_mutants_out / "caught.txt"),
            "missed_txt": str(resolved_mutants_out / "missed.txt"),
            "timeout_txt": str(resolved_mutants_out / "timeout.txt"),
            "unviable_txt": str(resolved_mutants_out / "unviable.txt"),
        },
    }

    out_path = args.run_dir / "triage.json"
    out_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    print(
        "Phase 0 mutation triage artifact generated: "
        f"{out_path} (survivors={len(survivors)}, tested={counts['tested_count']})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
