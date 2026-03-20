"""Tests for gate_eval module — expectation matching and pytest parsing."""

import json
from typing import TYPE_CHECKING

from tanren_core.gate_eval import (
    GateTestResult,
    _matches_any,  # noqa: PLC2701 — testing private implementation
    _normalize_test_name,  # noqa: PLC2701 — testing private implementation
    evaluate_gate,
    load_gate_expectations,
    parse_pytest_output,
)
from tanren_core.schemas import GateExpectation

if TYPE_CHECKING:
    from pathlib import Path


class TestEvaluateGateNoExpectations:
    def test_all_pass(self):
        results = [
            GateTestResult(name="test_a", passed=True),
            GateTestResult(name="test_b", passed=True),
        ]
        gate = evaluate_gate(results, None)
        assert gate.passed is True
        assert gate.must_pass_failures == []

    def test_some_fail(self):
        results = [
            GateTestResult(name="test_a", passed=True),
            GateTestResult(name="test_b", passed=False),
        ]
        gate = evaluate_gate(results, None)
        assert gate.passed is False
        assert gate.must_pass_failures == ["test_b"]

    def test_empty_results(self):
        gate = evaluate_gate([], None)
        assert gate.passed is True


class TestEvaluateGateWithExpectations:
    def test_must_pass_all_pass(self):
        exp = GateExpectation(must_pass=["lint", "unit:foo"])
        results = [
            GateTestResult(name="lint", passed=True),
            GateTestResult(name="unit:foo::test_a", passed=True),
        ]
        gate = evaluate_gate(results, exp)
        assert gate.passed is True
        assert gate.must_pass_failures == []

    def test_must_pass_failure(self):
        exp = GateExpectation(must_pass=["lint", "unit:foo"])
        results = [
            GateTestResult(name="lint", passed=True),
            GateTestResult(name="unit:foo::test_a", passed=False),
        ]
        gate = evaluate_gate(results, exp)
        assert gate.passed is False
        assert "unit:foo::test_a" in gate.must_pass_failures

    def test_expect_fail_actually_fails(self):
        exp = GateExpectation(
            must_pass=["lint"],
            expect_fail=["integration:bar"],
        )
        results = [
            GateTestResult(name="lint", passed=True),
            GateTestResult(name="integration:bar::test_x", passed=False),
        ]
        gate = evaluate_gate(results, exp)
        assert gate.passed is True
        assert gate.unexpected_passes == []

    def test_expect_fail_unexpectedly_passes(self):
        exp = GateExpectation(
            must_pass=["lint"],
            expect_fail=["integration:bar"],
        )
        results = [
            GateTestResult(name="lint", passed=True),
            GateTestResult(name="integration:bar::test_x", passed=True),
        ]
        gate = evaluate_gate(results, exp)
        assert gate.passed is True  # Not a failure, just a warning
        assert "integration:bar::test_x" in gate.unexpected_passes

    def test_skip_ignored(self):
        exp = GateExpectation(
            must_pass=["lint"],
            skip=["unit:baz"],
        )
        results = [
            GateTestResult(name="lint", passed=True),
            GateTestResult(name="unit:baz::test_y", passed=False),  # Skipped, ignored
        ]
        gate = evaluate_gate(results, exp)
        assert gate.passed is True
        assert gate.must_pass_failures == []

    def test_unlisted_failure_conservative(self):
        exp = GateExpectation(must_pass=["lint"])
        results = [
            GateTestResult(name="lint", passed=True),
            GateTestResult(name="unknown_test", passed=False),  # Not in any list
        ]
        gate = evaluate_gate(results, exp)
        assert gate.passed is False
        assert "unknown_test" in gate.must_pass_failures

    def test_wildcard_must_pass(self):
        exp = GateExpectation(must_pass=["*"])
        results = [
            GateTestResult(name="lint", passed=True),
            GateTestResult(name="unit:foo::test_a", passed=True),
            GateTestResult(name="integration:bar::test_b", passed=False),
        ]
        gate = evaluate_gate(results, exp)
        assert gate.passed is False
        assert "integration:bar::test_b" in gate.must_pass_failures

    def test_wildcard_all_pass(self):
        exp = GateExpectation(must_pass=["*"])
        results = [
            GateTestResult(name="lint", passed=True),
            GateTestResult(name="unit:foo", passed=True),
        ]
        gate = evaluate_gate(results, exp)
        assert gate.passed is True


class TestMatchesAny:
    def test_exact_match(self):
        assert _matches_any("lint", {"lint"}) is True

    def test_prefix_match_double_colon(self):
        assert _matches_any("unit:foo::test_bar", {"unit:foo"}) is True

    def test_prefix_match_single_colon(self):
        assert _matches_any("unit:foo:sub", {"unit:foo"}) is True

    def test_no_match(self):
        assert _matches_any("unit:bar::test", {"unit:foo"}) is False

    def test_partial_no_match(self):
        assert _matches_any("unit:foobar", {"unit:foo"}) is False

    def test_empty_patterns(self):
        assert _matches_any("anything", set()) is False


class TestParsePytestOutput:
    def test_verbose_output(self):
        stdout = (
            "tests/unit/test_foo.py::test_bar PASSED\n"
            "tests/unit/test_foo.py::test_baz FAILED\n"
            "tests/integration/test_pipe.py::TestClass::test_method PASSED\n"
        )
        results = parse_pytest_output(stdout)
        assert len(results) == 3
        assert results[0].name == "unit:test_foo::test_bar"
        assert results[0].passed is True
        assert results[1].name == "unit:test_foo::test_baz"
        assert results[1].passed is False
        assert results[2].name == "integration:test_pipe::TestClass::test_method"
        assert results[2].passed is True

    def test_error_result(self):
        stdout = "tests/unit/test_foo.py::test_bar ERROR\n"
        results = parse_pytest_output(stdout)
        assert len(results) == 1
        assert results[0].passed is False

    def test_empty_output(self):
        assert parse_pytest_output("") == []

    def test_noise_ignored(self):
        stdout = (
            "===== test session starts =====\n"
            "collected 3 items\n"
            "tests/unit/test_foo.py::test_bar PASSED\n"
            "===== 1 passed =====\n"
        )
        results = parse_pytest_output(stdout)
        assert len(results) == 1


class TestNormalizeTestName:
    def test_simple(self):
        assert _normalize_test_name("tests/unit/test_foo.py::test_bar") == "unit:test_foo::test_bar"

    def test_nested(self):
        assert _normalize_test_name("tests/integration/sub/test_x.py::TestClass::test_method") == (
            "integration:sub:test_x::TestClass::test_method"
        )

    def test_no_tests_prefix(self):
        assert _normalize_test_name("unit/test_foo.py::test_bar") == "unit:test_foo::test_bar"


class TestLoadGateExpectations:
    def test_valid(self, tmp_path: Path):
        data = [
            {"task_id": 1, "title": "Task 1", "gate": {"must_pass": ["lint"]}},
            {"task_id": 2, "title": "Task 2", "gate": {"must_pass": ["lint", "unit:foo"]}},
        ]
        (tmp_path / "gate-expectations.json").write_text(json.dumps(data))
        result = load_gate_expectations(tmp_path, 2)
        assert result is not None
        assert result.must_pass == ["lint", "unit:foo"]

    def test_task_not_found(self, tmp_path: Path):
        data = [{"task_id": 1, "title": "Task 1", "gate": {"must_pass": ["lint"]}}]
        (tmp_path / "gate-expectations.json").write_text(json.dumps(data))
        assert load_gate_expectations(tmp_path, 99) is None

    def test_missing_file(self, tmp_path: Path):
        assert load_gate_expectations(tmp_path, 1) is None

    def test_malformed_json(self, tmp_path: Path):
        (tmp_path / "gate-expectations.json").write_text("bad")
        assert load_gate_expectations(tmp_path, 1) is None
