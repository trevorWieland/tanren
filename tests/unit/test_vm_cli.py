"""Tests for VM CLI commands."""

from __future__ import annotations

import os
from unittest.mock import AsyncMock, patch

from typer.testing import CliRunner

from tanren_cli.vm_cli import vm
from tanren_core.adapters.remote_types import VMAssignment
from tanren_core.config import Config


def _mock_config(tmp_path=None) -> Config:
    base = str(tmp_path) if tmp_path else "/tmp"
    return Config(
        ipc_dir=f"{base}/ipc",
        github_dir=f"{base}/github",
        data_dir=f"{base}/data",
        worktree_registry_path=f"{base}/data/worktrees.json",
    )


def _make_assignment(**overrides) -> VMAssignment:
    defaults = {
        "vm_id": "vm-001",
        "workflow_id": "wf-abc-123",
        "project": "myproject",
        "spec": "default",
        "host": "10.0.0.1",
        "assigned_at": "2025-01-15T10:00:00Z",
    }
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

        with (
            patch("tanren_cli.vm_cli._load_config", return_value=_mock_config()),
            patch("tanren_cli.vm_cli._get_state_store", return_value=store),
        ):
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

        with (
            patch("tanren_cli.vm_cli._load_config", return_value=_mock_config()),
            patch("tanren_cli.vm_cli._get_state_store", return_value=store),
        ):
            runner = CliRunner()
            result = runner.invoke(vm, ["list"])

        assert result.exit_code == 0
        assert "No active VM assignments." in result.output

    def test_output_format_has_header_and_separator(self):
        store = _mock_store()
        store.get_active_assignments.return_value = [_make_assignment()]

        with (
            patch("tanren_cli.vm_cli._load_config", return_value=_mock_config()),
            patch("tanren_cli.vm_cli._get_state_store", return_value=store),
        ):
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

        with (
            patch("tanren_cli.vm_cli._load_config", return_value=_mock_config()),
            patch("tanren_cli.vm_cli._get_state_store", return_value=store),
        ):
            runner = CliRunner()
            result = runner.invoke(vm, ["release", "vm-001"])

        assert result.exit_code == 0
        assert "Released VM vm-001" in result.output
        assert "wf-abc-123" in result.output
        store.record_release.assert_awaited_once_with("vm-001")

    def test_exits_error_for_unknown_vm(self):
        store = _mock_store()
        store.get_assignment.return_value = None

        with (
            patch("tanren_cli.vm_cli._load_config", return_value=_mock_config()),
            patch("tanren_cli.vm_cli._get_state_store", return_value=store),
        ):
            runner = CliRunner()
            result = runner.invoke(vm, ["release", "vm-unknown"])

        assert result.exit_code != 0
        assert "No active assignment found for VM: vm-unknown" in result.output


class TestVmRecover:
    def test_shows_empty_message(self):
        store = _mock_store()
        store.get_active_assignments.return_value = []

        with (
            patch("tanren_cli.vm_cli._load_config", return_value=_mock_config()),
            patch("tanren_cli.vm_cli._get_state_store", return_value=store),
        ):
            runner = CliRunner()
            result = runner.invoke(vm, ["recover"])

        assert result.exit_code == 0
        assert "No active assignments to recover." in result.output


class TestVmDryRun:
    def test_prints_dry_run_without_connecting(self, tmp_path):
        github_dir = tmp_path / "github"
        project_dir = github_dir / "myproject"
        project_dir.mkdir(parents=True)
        (project_dir / ".env").write_text("API_KEY=abc\n")
        (project_dir / "tanren.yml").write_text(
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "environment:\n"
            "  default:\n"
            "    type: remote\n"
            "    server_type: cpx31\n"
            "    setup:\n"
            "      - make setup\n"
        )
        remote_cfg = tmp_path / "remote.yml"
        remote_cfg.write_text(
            "provisioner:\n"
            "  type: manual\n"
            "  settings:\n"
            "    vms:\n"
            "      - id: vm-1\n"
            "        host: 10.0.0.1\n"
            "repos:\n"
            "  myproject: https://github.com/org/myproject.git\n"
        )

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(github_dir),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            remote_config_path=str(remote_cfg),
        )
        with patch("tanren_cli.vm_cli._load_config", return_value=config):
            runner = CliRunner()
            result = runner.invoke(
                vm,
                ["dry-run", "--project", "myproject", "--environment-profile", "default"],
            )

        assert result.exit_code == 0
        assert "provisioner: manual" in result.output
        assert "repo_clone: https://github.com/org/myproject.git" in result.output
        assert "setup_commands:" in result.output

    def test_uses_hetzner_server_type_override(self, tmp_path):
        github_dir = tmp_path / "github"
        project_dir = github_dir / "myproject"
        project_dir.mkdir(parents=True)
        (project_dir / "tanren.yml").write_text(
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "environment:\n"
            "  default:\n"
            "    type: remote\n"
            "    server_type: cpx31\n"
        )
        remote_cfg = tmp_path / "remote.yml"
        remote_cfg.write_text(
            "provisioner:\n"
            "  type: hetzner\n"
            "  settings:\n"
            "    token_env: HCLOUD_TOKEN\n"
            "    default_server_type: cpx21\n"
            "    location: ash\n"
            "    image: ubuntu-24.04\n"
            "    ssh_key_name: tanren\n"
        )

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(github_dir),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            remote_config_path=str(remote_cfg),
        )
        with patch("tanren_cli.vm_cli._load_config", return_value=config):
            runner = CliRunner()
            result = runner.invoke(
                vm,
                ["dry-run", "--project", "myproject", "--environment-profile", "default"],
            )

        assert result.exit_code == 0
        assert "provisioner: hetzner" in result.output
        assert "server_type: cpx31 (profile.server_type)" in result.output

    def test_dry_run_does_not_mutate_process_env(self, tmp_path, monkeypatch):
        github_dir = tmp_path / "github"
        project_dir = github_dir / "myproject"
        project_dir.mkdir(parents=True)
        secrets_file = tmp_path / "dev-secrets.env"
        secrets_file.write_text("HCLOUD_TOKEN=from-file\n")
        remote_cfg = tmp_path / "remote.yml"
        remote_cfg.write_text(
            "provisioner:\n"
            "  type: manual\n"
            "  settings:\n"
            "    vms:\n"
            "      - id: vm-1\n"
            "        host: 10.0.0.1\n"
            "secrets:\n"
            f"  developer_secrets_path: {secrets_file}\n"
        )
        monkeypatch.delenv("HCLOUD_TOKEN", raising=False)

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(github_dir),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            remote_config_path=str(remote_cfg),
        )
        with patch("tanren_cli.vm_cli._load_config", return_value=config):
            result = CliRunner().invoke(
                vm,
                ["dry-run", "--project", "myproject", "--environment-profile", "default"],
            )

        assert result.exit_code == 0
        assert os.environ.get("HCLOUD_TOKEN") is None
