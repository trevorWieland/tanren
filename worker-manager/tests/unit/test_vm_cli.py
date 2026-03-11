"""Tests for VM CLI commands."""

from __future__ import annotations

from unittest.mock import AsyncMock, patch

from typer.testing import CliRunner

from worker_manager.adapters.remote_types import VMAssignment
from worker_manager.vm_cli import vm


def _make_assignment(**overrides) -> VMAssignment:
    defaults = dict(
        vm_id="vm-001",
        workflow_id="wf-abc-123",
        project="myproject",
        spec="default",
        host="10.0.0.1",
        assigned_at="2025-01-15T10:00:00Z",
    )
    defaults.update(overrides)
    return VMAssignment(**defaults)


def _mock_store():
    store = AsyncMock()
    store.get_active_assignments = AsyncMock(return_value=[])
    store.get_assignment = AsyncMock(return_value=None)
    store.record_release = AsyncMock()
    store.close = AsyncMock()
    return store


class TestVmList:
    def test_shows_active_assignments(self):
        store = _mock_store()
        store.get_active_assignments.return_value = [
            _make_assignment(),
            _make_assignment(vm_id="vm-002", host="10.0.0.2"),
        ]

        with patch("worker_manager.vm_cli._get_state_store", return_value=store):
            runner = CliRunner()
            result = runner.invoke(vm, ["list"])

        assert result.exit_code == 0
        assert "vm-001" in result.output
        assert "vm-002" in result.output
        assert "10.0.0.1" in result.output
        assert "10.0.0.2" in result.output

    def test_shows_empty_message(self):
        store = _mock_store()
        store.get_active_assignments.return_value = []

        with patch("worker_manager.vm_cli._get_state_store", return_value=store):
            runner = CliRunner()
            result = runner.invoke(vm, ["list"])

        assert result.exit_code == 0
        assert "No active VM assignments." in result.output

    def test_output_format_has_header_and_separator(self):
        store = _mock_store()
        store.get_active_assignments.return_value = [_make_assignment()]

        with patch("worker_manager.vm_cli._get_state_store", return_value=store):
            runner = CliRunner()
            result = runner.invoke(vm, ["list"])

        lines = result.output.strip().splitlines()
        assert "VM ID" in lines[0]
        assert "Host" in lines[0]
        assert "Workflow" in lines[0]
        assert "Project" in lines[0]
        assert "Assigned At" in lines[0]
        assert lines[1].startswith("-" * 50)


class TestVmRelease:
    def test_releases_known_vm(self):
        store = _mock_store()
        assignment = _make_assignment()
        store.get_assignment.return_value = assignment

        with patch("worker_manager.vm_cli._get_state_store", return_value=store):
            runner = CliRunner()
            result = runner.invoke(vm, ["release", "vm-001"])

        assert result.exit_code == 0
        assert "Released VM vm-001" in result.output
        assert "wf-abc-123" in result.output
        store.record_release.assert_awaited_once_with("vm-001")

    def test_exits_error_for_unknown_vm(self):
        store = _mock_store()
        store.get_assignment.return_value = None

        with patch("worker_manager.vm_cli._get_state_store", return_value=store):
            runner = CliRunner()
            result = runner.invoke(vm, ["release", "vm-unknown"])

        assert result.exit_code != 0
        assert "No active assignment found for VM: vm-unknown" in result.output


class TestVmRecover:
    def test_shows_empty_message(self):
        store = _mock_store()
        store.get_active_assignments.return_value = []

        with patch("worker_manager.vm_cli._get_state_store", return_value=store):
            runner = CliRunner()
            result = runner.invoke(vm, ["recover"])

        assert result.exit_code == 0
        assert "No active assignments to recover." in result.output
