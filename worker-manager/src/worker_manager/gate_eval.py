"""Gate evaluation with task-level expectations and test output parsing."""

import json
import re
from pathlib import Path

from pydantic import BaseModel, ConfigDict, Field

from worker_manager.schemas import GateExpectation, GateResult


class GateTestResult(BaseModel):
    """Normalized result of a single test or check."""

    model_config = ConfigDict(extra="forbid")

    name: str = Field(...)
    passed: bool = Field(...)
    output: str = Field(default="")


def evaluate_gate(
    test_results: list[GateTestResult],
    expectations: GateExpectation | None,
) -> GateResult:
    """Evaluate test results against task-level gate expectations.

    If expectations is None, falls back to binary: any failure = gate failure.
    """
    if expectations is None:
        failures = [t.name for t in test_results if not t.passed]
        return GateResult(
            attempt=0,
            passed=len(failures) == 0,
            must_pass_failures=failures,
        )

    must_pass_failures: list[str] = []
    unexpected_passes: list[str] = []
    skip_set = set(expectations.skip)
    expect_fail_set = set(expectations.expect_fail)
    must_pass_set = set(expectations.must_pass)

    for test in test_results:
        if _matches_any(test.name, skip_set):
            continue

        if _matches_any(test.name, expect_fail_set):
            if test.passed:
                unexpected_passes.append(test.name)
            continue

        if "*" in must_pass_set or _matches_any(test.name, must_pass_set):
            if not test.passed:
                must_pass_failures.append(test.name)
            continue

        # Unlisted test: treat as must_pass (conservative default)
        if not test.passed:
            must_pass_failures.append(test.name)

    return GateResult(
        attempt=0,
        passed=len(must_pass_failures) == 0,
        must_pass_failures=must_pass_failures,
        unexpected_passes=unexpected_passes,
    )


def _matches_any(test_name: str, patterns: set[str]) -> bool:
    """Prefix matching: 'unit:module_a' matches 'unit:module_a::test_foo'."""
    for p in patterns:
        if test_name == p:
            return True
        if test_name.startswith(p + "::") or test_name.startswith(p + ":"):
            return True
    return False


def parse_pytest_output(stdout: str) -> list[GateTestResult]:
    """Parse pytest verbose output into normalized GateTestResult list.

    Handles pytest -v format:
      tests/test_foo.py::test_bar PASSED
      tests/test_foo.py::test_baz FAILED
    """
    results: list[GateTestResult] = []

    for match in re.finditer(
        r"^([\w/._-]+(?:::[\w._-]+)+)\s+(PASSED|FAILED|ERROR)",
        stdout,
        re.MULTILINE,
    ):
        name = _normalize_test_name(match.group(1))
        passed = match.group(2) == "PASSED"
        results.append(GateTestResult(name=name, passed=passed))

    return results


def _normalize_test_name(raw: str) -> str:
    """Normalize pytest test name to prefix-matchable format.

    tests/unit/test_foo.py::TestClass::test_method
    → unit:test_foo::TestClass::test_method
    """
    raw = re.sub(r"^tests/", "", raw)
    raw = re.sub(r"\.py::", "::", raw)
    raw = raw.replace("/", ":")
    return raw


def load_gate_expectations(spec_folder_path: Path, task_id: int) -> GateExpectation | None:
    """Load gate expectations for a specific task from gate-expectations.json."""
    path = spec_folder_path / "gate-expectations.json"
    if not path.exists():
        return None
    try:
        data = json.loads(path.read_text())
        for entry in data:
            if entry.get("task_id") == task_id:
                return GateExpectation.model_validate(entry.get("gate", {}))
    except Exception:
        pass
    return None
