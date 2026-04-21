"""Fail CI when redaction benchmark means exceed configured thresholds."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def load_thresholds(path: Path) -> dict[str, float]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    scenarios = payload.get("scenarios")
    if not isinstance(scenarios, dict):
        raise ValueError("threshold file must contain a 'scenarios' object")

    thresholds: dict[str, float] = {}
    for scenario, value in scenarios.items():
        if not isinstance(scenario, str) or not isinstance(value, (int, float)):
            raise ValueError("scenario thresholds must be string -> numeric")
        thresholds[scenario] = float(value)
    return thresholds


def read_benchmark_mean_ns(criterion_dir: Path, scenario: str) -> float:
    parts = scenario.split("/")
    candidate_paths = [
        criterion_dir.joinpath(*parts, "new", "estimates.json"),
        criterion_dir.joinpath(scenario.replace("/", "_"), "new", "estimates.json"),
    ]
    if len(parts) >= 3:
        candidate_paths.append(
            criterion_dir.joinpath("_".join(parts[:-1]), parts[-1], "new", "estimates.json")
        )
    if len(parts) == 2:
        candidate_paths.append(
            criterion_dir.joinpath("_".join(parts), "new", "estimates.json")
        )
    estimate_path = next((path for path in candidate_paths if path.exists()), None)
    if estimate_path is None:
        raise FileNotFoundError(
            f"missing criterion output for '{scenario}' (checked: {candidate_paths})"
        )

    payload = json.loads(estimate_path.read_text(encoding="utf-8"))
    mean = payload.get("mean")
    if not isinstance(mean, dict) or not isinstance(mean.get("point_estimate"), (int, float)):
        raise ValueError(f"invalid estimate payload in {estimate_path}")
    return float(mean["point_estimate"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--thresholds",
        type=Path,
        default=Path("crates/tanren-runtime/benches/baselines/redaction-thresholds.json"),
        help="path to scenario mean thresholds (nanoseconds)",
    )
    parser.add_argument(
        "--criterion-dir",
        type=Path,
        default=Path("target/criterion"),
        help="criterion output root directory",
    )
    args = parser.parse_args()

    thresholds = load_thresholds(args.thresholds)

    failures: list[str] = []
    for scenario, max_mean_ns in thresholds.items():
        observed_mean_ns = read_benchmark_mean_ns(args.criterion_dir, scenario)
        status = "PASS" if observed_mean_ns <= max_mean_ns else "FAIL"
        print(
            f"[{status}] {scenario}: observed={observed_mean_ns:.0f}ns max={max_mean_ns:.0f}ns"
        )
        if observed_mean_ns > max_mean_ns:
            failures.append(
                f"{scenario} observed={observed_mean_ns:.0f}ns exceeds max={max_mean_ns:.0f}ns"
            )

    if failures:
        print("\nRedaction performance regression gate failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1

    print("\nRedaction performance gate passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
