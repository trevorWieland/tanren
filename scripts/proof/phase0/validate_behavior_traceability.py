#!/usr/bin/env python3
# ruff: noqa: D103
"""Validate Phase 0 behavior-to-scenario traceability inventory."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

SCENARIO_HEADING = re.compile(r"^### Scenario (?P<id>\d+\.\d+): (?P<title>.+)$")
BEHAVIOR_ID_PATTERN = re.compile(r"^BEH-P0-\d{3}$")
EXPECTED_OBLIGATIONS = {"positive", "falsification"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifact",
        type=Path,
        default=Path("docs/rewrite/PHASE0_BEHAVIOR_TRACEABILITY.json"),
        help="Path to behavior traceability JSON artifact.",
    )
    parser.add_argument(
        "--bdd-source",
        type=Path,
        default=Path("docs/rewrite/PHASE0_PROOF_BDD.md"),
        help="Path to Phase 0 BDD source markdown.",
    )
    return parser.parse_args()


def load_bdd_scenarios(path: Path) -> dict[str, str]:
    scenarios: dict[str, str] = {}
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        match = SCENARIO_HEADING.match(line)
        if not match:
            continue
        scenario_id = match.group("id")
        if scenario_id in scenarios:
            raise ValueError(f"duplicate scenario heading in BDD source: {scenario_id}")
        scenarios[scenario_id] = match.group("title")
    return scenarios


def scenario_sort_key(scenario_id: str) -> tuple[int, int]:
    major_str, minor_str = scenario_id.split(".", maxsplit=1)
    return int(major_str), int(minor_str)


def main() -> int:
    args = parse_args()
    errors: list[str] = []

    bdd_scenarios = load_bdd_scenarios(args.bdd_source)
    if not bdd_scenarios:
        errors.append(f"{args.bdd_source}: found no scenario headings")

    payload = json.loads(args.artifact.read_text(encoding="utf-8"))
    inventory = payload.get("behavior_inventory")
    if not isinstance(inventory, list):
        errors.append("behavior_inventory must be a list")
        inventory = []

    found_scenarios: set[str] = set()
    found_behaviors: set[str] = set()
    order: list[str] = []

    for index, row in enumerate(inventory):
        row_prefix = f"entry[{index}]"
        if not isinstance(row, dict):
            errors.append(f"{row_prefix}: must be an object")
            continue

        behavior_id = row.get("behavior_id")
        scenario_id = row.get("scenario_id")
        scenario_title = row.get("scenario_title")
        owner = row.get("owner")
        obligations = row.get("obligations")

        if not isinstance(behavior_id, str) or not BEHAVIOR_ID_PATTERN.fullmatch(behavior_id):
            errors.append(f"{row_prefix}: invalid behavior_id {behavior_id!r}")
        elif behavior_id in found_behaviors:
            errors.append(f"{row_prefix}: duplicate behavior_id {behavior_id}")
        else:
            found_behaviors.add(behavior_id)

        if not isinstance(scenario_id, str):
            errors.append(f"{row_prefix}: scenario_id must be a string")
            continue
        order.append(scenario_id)
        if scenario_id in found_scenarios:
            errors.append(f"{row_prefix}: duplicate scenario_id {scenario_id}")
        else:
            found_scenarios.add(scenario_id)
        if scenario_id not in bdd_scenarios:
            errors.append(f"{row_prefix}: scenario_id {scenario_id} missing in BDD source")

        expected_title = bdd_scenarios.get(scenario_id)
        if isinstance(expected_title, str) and scenario_title != expected_title:
            errors.append(
                f"{row_prefix}: scenario_title mismatch for {scenario_id}:"
                f" expected {expected_title!r}, got {scenario_title!r}"
            )

        if not isinstance(owner, str) or not owner.strip():
            errors.append(f"{row_prefix}: owner must be a non-empty string")

        if not isinstance(obligations, dict):
            errors.append(f"{row_prefix}: obligations must be an object")
            continue

        obligation_keys = set(obligations.keys())
        if obligation_keys != EXPECTED_OBLIGATIONS:
            errors.append(
                f"{row_prefix}: obligations keys must be {sorted(EXPECTED_OBLIGATIONS)},"
                f" got {sorted(obligation_keys)}"
            )

        for witness in sorted(EXPECTED_OBLIGATIONS):
            obligation = obligations.get(witness)
            obligation_prefix = f"{row_prefix}.obligations[{witness}]"
            if not isinstance(obligation, dict):
                errors.append(f"{obligation_prefix}: must be an object")
                continue

            if obligation.get("witness") != witness:
                errors.append(
                    f"{obligation_prefix}: witness must be {witness!r},"
                    f" got {obligation.get('witness')!r}"
                )

            expected_tag = f"@{behavior_id}" if isinstance(behavior_id, str) else None
            if expected_tag is not None and obligation.get("tag") != expected_tag:
                errors.append(
                    f"{obligation_prefix}: tag must be {expected_tag!r},"
                    f" got {obligation.get('tag')!r}"
                )

            if obligation.get("scenario_ref") != scenario_id:
                errors.append(
                    f"{obligation_prefix}: scenario_ref must be {scenario_id!r},"
                    f" got {obligation.get('scenario_ref')!r}"
                )

            feature_file = obligation.get("planned_feature_file")
            if not isinstance(feature_file, str) or not feature_file.endswith(".feature"):
                errors.append(f"{obligation_prefix}: planned_feature_file must end with .feature")

    missing = set(bdd_scenarios) - found_scenarios
    if missing:
        errors.append(f"missing scenario_ids: {sorted(missing, key=scenario_sort_key)}")

    if order != sorted(order, key=scenario_sort_key):
        errors.append("behavior_inventory must be sorted by scenario_id")

    if errors:
        print("Phase 0 behavior traceability validation failed:")
        for err in errors:
            print(f"- {err}")
        return 1

    print(
        "Phase 0 behavior traceability validation passed: "
        f"{len(found_behaviors)} behavior IDs across {len(found_scenarios)} scenarios."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
