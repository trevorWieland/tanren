"""Regression tests for Phase 0 mutation-stage and triage script behavior."""

from __future__ import annotations

import json
import os
import stat
import subprocess
from pathlib import Path


def _write_executable(path: Path, content: str) -> None:
    path.write_text(content)
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def _write_traceability(path: Path) -> None:
    path.write_text(
        json.dumps(
            {
                "behavior_inventory": [
                    {"behavior_id": "BEH-P0-101", "wave": "A"},
                    {"behavior_id": "BEH-P0-601", "wave": "B"},
                    {"behavior_id": "BEH-P0-801", "wave": "C"},
                ]
            }
        )
    )


def _seed_nested_mutants_out(mutants_out_root: Path) -> None:
    nested = mutants_out_root / "mutants.out"
    nested.mkdir(parents=True, exist_ok=True)
    (nested / "outcomes.json").write_text(
        json.dumps(
            {
                "total_mutants": 5,
                "success": 0,
                "caught": 0,
                "missed": 2,
                "timeout": 0,
                "unviable": 3,
                "outcomes": [
                    {
                        "summary": "Missed",
                        "scenario": {
                            "Mutant": {
                                "name": (
                                    "crates/tanren-bdd-phase0/src/main.rs:148:8: "
                                    "delete ! in submit_lifecycle_transition"
                                )
                            }
                        },
                    },
                    {
                        "summary": "Missed",
                        "scenario": {
                            "Mutant": {
                                "name": (
                                    "crates/tanren-bdd-phase0/src/main.rs:156:30: "
                                    "replace += with -= in submit_lifecycle_transition"
                                )
                            }
                        },
                    },
                    {"summary": "Unviable"},
                    {"summary": "Unviable"},
                    {"summary": "Unviable"},
                ],
            }
        )
    )


def test_render_mutation_triage_handles_nested_outcomes_json_layout(tmp_path: Path) -> None:
    run_dir = tmp_path / "run"
    run_dir.mkdir(parents=True, exist_ok=True)
    traceability = tmp_path / "traceability.json"
    _write_traceability(traceability)
    mutants_out_root = run_dir / "mutants.out"
    _seed_nested_mutants_out(mutants_out_root)

    script = Path("scripts/proof/phase0/render_mutation_triage.py")
    result = subprocess.run(
        [
            "uv",
            "run",
            "python",
            str(script),
            "--traceability",
            str(traceability),
            "--run-dir",
            str(run_dir),
            "--mutants-out",
            str(mutants_out_root),
            "--status",
            "executed_nonzero",
            "--exit-code",
            "2",
            "--version",
            "cargo-mutants 99",
            "--shard",
            "0/32",
        ],
        cwd=Path(__file__).resolve().parents[2],
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr

    triage = json.loads((run_dir / "triage.json").read_text())
    assert triage["outcomes"]["missed_count"] == 2
    assert triage["outcomes"]["unviable_count"] == 3
    assert triage["outcomes"]["tested_count"] == 5
    assert triage["survivors"][0]["outcome"] == "missed"
    assert "submit_lifecycle_transition" in triage["survivors"][0]["mutant"]
    assert triage["artifacts"]["requested_mutants_out"].endswith("/run/mutants.out")
    assert triage["artifacts"]["mutants_out"].endswith("/run/mutants.out/mutants.out")


def test_run_mutation_stage_prints_strict_failure_summary_with_missed_mutants(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir(parents=True, exist_ok=True)
    _write_executable(
        fake_bin / "cargo-mutants",
        """#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--version" ]]; then
  echo "cargo-mutants 99.0.0"
  exit 0
fi

out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      out="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

mkdir -p "${out}/mutants.out"
cat > "${out}/mutants.out/outcomes.json" <<'JSON'
{
  "total_mutants": 5,
  "success": 0,
  "caught": 0,
  "missed": 2,
  "timeout": 0,
  "unviable": 3,
  "outcomes": [
    {"summary": "Missed", "scenario": {"Mutant": {"name": "crates/tanren-bdd-phase0/src/main.rs:148:8: delete ! in submit_lifecycle_transition"}}},
    {"summary": "Missed", "scenario": {"Mutant": {"name": "crates/tanren-bdd-phase0/src/main.rs:156:30: replace += with -= in submit_lifecycle_transition"}}},
    {"summary": "Unviable"},
    {"summary": "Unviable"},
    {"summary": "Unviable"}
  ]
}
JSON
echo "Found 5 mutants to test"
echo "5 mutants tested in 7s: 2 missed, 3 unviable"
exit 2
""",
    )

    traceability = tmp_path / "traceability.json"
    _write_traceability(traceability)
    output_root = tmp_path / "mutation-artifacts"
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}:{env['PATH']}"
    env["PHASE0_MUTATION_ENFORCE"] = "1"
    env["PHASE0_MUTATION_OUTPUT_ROOT"] = str(output_root)
    env["PHASE0_BEHAVIOR_TRACEABILITY_FILE"] = str(traceability)

    result = subprocess.run(
        ["bash", "scripts/proof/phase0/run_mutation_stage.sh"],
        cwd=Path(__file__).resolve().parents[2],
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 2
    assert "Phase 0 mutation gate summary: tested=5 missed=2 unviable=3" in result.stdout
    assert "Phase 0 mutation missed mutants (first 5):" in result.stdout
    assert "submit_lifecycle_transition" in result.stdout
