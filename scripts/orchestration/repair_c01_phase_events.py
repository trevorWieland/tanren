#!/usr/bin/env python3
# ruff: noqa: DOC201, DOC501
"""One-off canonical rewrite for spec c01 phase-events log."""

from __future__ import annotations

import argparse
import json
import shutil
from datetime import UTC, datetime
from pathlib import Path

SPEC_ID = "00000000-0000-0000-0000-000000000c01"
DEFAULT_PATH = "tanren/specs/rust-testing-hard-cutover-phase0/phase-events.jsonl"


def parse_args() -> argparse.Namespace:
    """Parse CLI args for the one-off c01 repair command."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--path",
        type=Path,
        default=Path(DEFAULT_PATH),
        help="phase-events.jsonl path for c01",
    )
    parser.add_argument(
        "--spec-id",
        default=SPEC_ID,
        help="expected spec id for validation",
    )
    return parser.parse_args()


def canonical_line(raw: str, expected_spec_id: str, line_no: int) -> str:
    """Normalize one JSONL row and enforce spec-id consistency."""
    obj = json.loads(raw)
    if obj.get("spec_id") != expected_spec_id:
        raise ValueError(
            f"line {line_no}: expected spec_id={expected_spec_id}, got {obj.get('spec_id')}"
        )
    obj["schema_version"] = "1.0.0"
    return json.dumps(obj, sort_keys=True, separators=(",", ":"))


def main() -> int:
    """Rewrite c01 phase-events in-place after creating a timestamped backup."""
    args = parse_args()
    path = args.path
    if not path.is_file():
        raise FileNotFoundError(path)

    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    backup_dir = path.parent / "orchestration" / "phase-events-repair-backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    backup_path = backup_dir / f"phase-events.{timestamp}.pre-repair.jsonl"
    shutil.copy2(path, backup_path)

    rewritten: list[str] = []
    for idx, raw in enumerate(path.read_text().splitlines(), start=1):
        if not raw.strip():
            continue
        rewritten.append(canonical_line(raw, args.spec_id, idx))
    path.write_text("\n".join(rewritten) + "\n")

    print(
        json.dumps(
            {
                "schema_version": "1.0.0",
                "spec_id": args.spec_id,
                "input": str(path),
                "backup": str(backup_path),
                "lines_rewritten": len(rewritten),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
