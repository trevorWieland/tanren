"""Fail CI when redaction benchmark means exceed configured budgets."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class ScenarioBudget:
    """Budget for a single benchmark scenario."""

    baseline_mean_ns: float
    max_mean_ns: float


@dataclass(frozen=True)
class PerfBudget:
    """Aggregate benchmark budget configuration."""

    max_regression_pct: float
    scenarios: dict[str, ScenarioBudget]


def load_budget(path: Path) -> PerfBudget:
    """Load and validate the benchmark budget JSON document.

    Returns:
        Parsed performance budget.

    Raises:
        TypeError: Threshold payload contains invalid types.
        ValueError: Threshold payload contains invalid numeric constraints.
    """
    payload = json.loads(path.read_text(encoding="utf-8"))
    raw_max_regression_pct = payload.get("max_regression_pct")
    if not isinstance(raw_max_regression_pct, (int, float)):
        raise TypeError("threshold file must contain numeric 'max_regression_pct'")
    max_regression_pct = float(raw_max_regression_pct)
    if max_regression_pct < 0:
        raise ValueError("'max_regression_pct' must be >= 0")

    scenarios = payload.get("scenarios")
    if not isinstance(scenarios, dict):
        raise TypeError("threshold file must contain a 'scenarios' object")

    parsed: dict[str, ScenarioBudget] = {}
    for scenario, spec in scenarios.items():
        if not isinstance(scenario, str) or not isinstance(spec, dict):
            raise TypeError("scenario budget must be string -> object")

        baseline = spec.get("baseline_mean_ns")
        absolute = spec.get("max_mean_ns")
        if not isinstance(baseline, (int, float)) or not isinstance(absolute, (int, float)):
            raise TypeError(
                f"scenario '{scenario}' must define numeric baseline_mean_ns and max_mean_ns"
            )
        baseline_ns = float(baseline)
        max_mean_ns = float(absolute)
        if baseline_ns <= 0 or max_mean_ns <= 0:
            raise ValueError(f"scenario '{scenario}' budgets must be > 0")
        if max_mean_ns < baseline_ns:
            raise ValueError(f"scenario '{scenario}' max_mean_ns must be >= baseline_mean_ns")
        parsed[scenario] = ScenarioBudget(
            baseline_mean_ns=baseline_ns,
            max_mean_ns=max_mean_ns,
        )

    return PerfBudget(max_regression_pct=max_regression_pct, scenarios=parsed)


def read_benchmark_mean_ns(criterion_dir: Path, scenario: str) -> float:
    """Read one scenario mean estimate from Criterion output.

    Returns:
        Mean point estimate in nanoseconds.

    Raises:
        FileNotFoundError: No estimate file exists for the scenario.
        TypeError: Estimate payload is not in expected Criterion shape.
    """
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
        candidate_paths.append(criterion_dir.joinpath("_".join(parts), "new", "estimates.json"))
    estimate_path = next((path for path in candidate_paths if path.exists()), None)
    if estimate_path is None:
        raise FileNotFoundError(
            f"missing criterion output for '{scenario}' (checked: {candidate_paths})"
        )

    payload = json.loads(estimate_path.read_text(encoding="utf-8"))
    mean = payload.get("mean")
    if not isinstance(mean, dict) or not isinstance(mean.get("point_estimate"), (int, float)):
        raise TypeError(f"invalid estimate payload in {estimate_path}")
    return float(mean["point_estimate"])


def main() -> int:
    """Run the perf-gate comparison and return shell exit status.

    Returns:
        Exit status code (0 when budgets pass, 1 when violations exist).
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--thresholds",
        type=Path,
        default=Path("crates/tanren-runtime/benches/baselines/redaction-thresholds.json"),
        help="path to scenario performance budgets",
    )
    parser.add_argument(
        "--criterion-dir",
        type=Path,
        default=Path("target/criterion"),
        help="criterion output root directory",
    )
    args = parser.parse_args()

    budget = load_budget(args.thresholds)

    failures: list[str] = []
    for scenario, scenario_budget in budget.scenarios.items():
        observed_mean_ns = read_benchmark_mean_ns(args.criterion_dir, scenario)
        relative_limit_ns = scenario_budget.baseline_mean_ns * (
            1 + (budget.max_regression_pct / 100.0)
        )
        regression_pct = (
            ((observed_mean_ns / scenario_budget.baseline_mean_ns) - 1) * 100
            if scenario_budget.baseline_mean_ns > 0
            else 0.0
        )
        relative_ok = observed_mean_ns <= relative_limit_ns
        absolute_ok = observed_mean_ns <= scenario_budget.max_mean_ns
        status = "PASS" if relative_ok and absolute_ok else "FAIL"

        print(
            f"[{status}] {scenario}: observed={observed_mean_ns:.0f}ns "
            f"baseline={scenario_budget.baseline_mean_ns:.0f}ns "
            f"delta={regression_pct:+.1f}% "
            f"relative_max={relative_limit_ns:.0f}ns "
            f"absolute_max={scenario_budget.max_mean_ns:.0f}ns"
        )

        if not (relative_ok and absolute_ok):
            reasons = []
            if not relative_ok:
                reasons.append(
                    "relative regression "
                    f"{regression_pct:+.1f}% exceeds "
                    f"+{budget.max_regression_pct:.1f}%"
                )
            if not absolute_ok:
                reasons.append(
                    f"absolute mean {observed_mean_ns:.0f}ns exceeds "
                    f"{scenario_budget.max_mean_ns:.0f}ns"
                )
            failures.append(f"{scenario}: " + "; ".join(reasons))

    if failures:
        print("\nRedaction performance regression gate failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1

    print("\nRedaction performance gate passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
