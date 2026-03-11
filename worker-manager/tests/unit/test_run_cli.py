"""Tests for tanren run CLI commands."""

from __future__ import annotations

import json
import time
from datetime import UTC, datetime
from pathlib import Path
from unittest.mock import AsyncMock

from typer.testing import CliRunner

from worker_manager.adapters.remote_types import VMHandle, VMProvider, WorkspacePath
from worker_manager.adapters.ssh import SSHConfig
from worker_manager.adapters.types import EnvironmentHandle, PhaseResult, RemoteEnvironmentRuntime
from worker_manager.config import Config
from worker_manager.roles import AgentTool
from worker_manager.run_cli import (
    PersistedRunHandle,
    PersistedSSHDefaults,
    run,
)
from worker_manager.schemas import Cli, Outcome


def _config(tmp_path: Path) -> Config:
    return Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        remote_config_path=str(tmp_path / "remote.yml"),
    )


def _env_handle() -> EnvironmentHandle:
    vm = VMHandle(
        vm_id="vm-1",
        host="203.0.113.10",
        provider=VMProvider.HETZNER,
        created_at="2026-01-01T00:00:00Z",
        hourly_cost=0.5,
    )
    runtime = RemoteEnvironmentRuntime(
        vm_handle=vm,
        connection=AsyncMock(),
        workspace_path=WorkspacePath(path="/workspace/proj", project="proj", branch="main"),
        profile={"name": "default", "type": "remote"},
        teardown_commands=("make clean",),
        provision_start=time.monotonic(),
        workflow_id="run-proj-abc",
    )
    return EnvironmentHandle(
        env_id="env-123",
        worktree_path=Path("/workspace/proj"),
        branch="main",
        project="proj",
        runtime=runtime,
    )


def _persisted(config: Config) -> Path:
    path = Path(config.data_dir) / "run-handles"
    path.mkdir(parents=True, exist_ok=True)
    persisted = PersistedRunHandle(
        env_id="env-123",
        vm_id="vm-1",
        project="proj",
        branch="main",
        workflow_id="run-proj-abc",
        environment_profile="default",
        local_worktree_path=str(Path(config.github_dir) / "proj"),
        workspace_path="/workspace/proj",
        teardown_commands=("make clean",),
        provisioned_at_utc=datetime.now(UTC).isoformat(),
        vm_handle=VMHandle(
            vm_id="vm-1",
            host="203.0.113.10",
            provider=VMProvider.HETZNER,
            created_at="2026-01-01T00:00:00Z",
            hourly_cost=0.5,
        ),
        ssh_defaults=PersistedSSHDefaults(
            user="root",
            key_path="~/.ssh/id_rsa",
            port=22,
            connect_timeout=10,
        ),
    )
    handle_file = path / "env-123.json"
    handle_file.write_text(persisted.model_dump_json(indent=2))
    return handle_file


def test_run_provision_prints_and_saves_handle(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    env = AsyncMock()
    env.provision.return_value = _env_handle()
    env.ssh_defaults = SSHConfig(host="", user="root", key_path="~/.ssh/id_rsa", port=22)

    monkeypatch.setattr("worker_manager.run_cli._load_config", lambda: config)
    monkeypatch.setattr("worker_manager.run_cli._build_remote_execution_env", lambda cfg: env)

    result = CliRunner().invoke(
        run,
        ["provision", "--project", "proj", "--environment-profile", "default", "--branch", "main"],
    )

    assert result.exit_code == 0
    assert "env_id: env-123" in result.output
    assert "vm_id: vm-1" in result.output
    handle_file = Path(config.data_dir) / "run-handles" / "env-123.json"
    assert handle_file.exists()


def test_run_execute_loads_handle_and_prints_result(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    _persisted(config)
    env = AsyncMock()
    env.execute.return_value = PhaseResult(
        outcome=Outcome.SUCCESS,
        signal="ok",
        exit_code=0,
        stdout="done",
        duration_secs=5,
        preflight_passed=True,
        retries=0,
    )

    monkeypatch.setattr("worker_manager.run_cli._load_config", lambda: config)
    monkeypatch.setattr("worker_manager.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "worker_manager.run_cli._resolve_agent_tool",
        lambda config, phase: AgentTool(cli=Cli.CLAUDE),
    )

    result = CliRunner().invoke(
        run,
        [
            "execute",
            "--handle",
            "env-123",
            "--project",
            "proj",
            "--spec-path",
            "tanren/specs/s1",
            "--phase",
            "do-task",
        ],
    )

    assert result.exit_code == 0
    assert "outcome: success" in result.output
    env.execute.assert_awaited_once()


def test_run_teardown_removes_handle_and_calls_teardown(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    handle_file = _persisted(config)
    env = AsyncMock()

    monkeypatch.setattr("worker_manager.run_cli._load_config", lambda: config)
    monkeypatch.setattr("worker_manager.run_cli._build_remote_execution_env", lambda cfg: env)

    result = CliRunner().invoke(run, ["teardown", "--handle", "env-123"])

    assert result.exit_code == 0
    assert not handle_file.exists()
    env.teardown.assert_awaited_once()


def test_run_full_executes_in_order(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    env = AsyncMock()
    env.provision.return_value = _env_handle()
    env.execute.return_value = PhaseResult(
        outcome=Outcome.SUCCESS,
        signal="ok",
        exit_code=0,
        stdout="done",
        duration_secs=3,
        preflight_passed=True,
        retries=0,
    )
    env.ssh_defaults = SSHConfig(host="", user="root", key_path="~/.ssh/id_rsa", port=22)

    monkeypatch.setattr("worker_manager.run_cli._load_config", lambda: config)
    monkeypatch.setattr("worker_manager.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "worker_manager.run_cli._resolve_agent_tool",
        lambda config, phase: AgentTool(cli=Cli.CLAUDE),
    )

    result = CliRunner().invoke(
        run,
        [
            "full",
            "--project",
            "proj",
            "--environment-profile",
            "default",
            "--branch",
            "main",
            "--spec-path",
            "tanren/specs/s1",
            "--phase",
            "do-task",
        ],
    )

    assert result.exit_code == 0
    assert env.provision.await_count == 1
    assert env.execute.await_count == 1
    assert env.teardown.await_count == 1


def test_run_full_teardown_runs_even_on_execute_failure(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    env = AsyncMock()
    env.provision.return_value = _env_handle()
    env.execute.return_value = PhaseResult(
        outcome=Outcome.ERROR,
        signal=None,
        exit_code=1,
        stdout="boom",
        duration_secs=3,
        preflight_passed=True,
        retries=0,
    )
    env.ssh_defaults = SSHConfig(host="", user="root", key_path="~/.ssh/id_rsa", port=22)

    monkeypatch.setattr("worker_manager.run_cli._load_config", lambda: config)
    monkeypatch.setattr("worker_manager.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "worker_manager.run_cli._resolve_agent_tool",
        lambda config, phase: AgentTool(cli=Cli.CLAUDE),
    )

    result = CliRunner().invoke(
        run,
        [
            "full",
            "--project",
            "proj",
            "--environment-profile",
            "default",
            "--branch",
            "main",
            "--spec-path",
            "tanren/specs/s1",
            "--phase",
            "do-task",
        ],
    )

    assert result.exit_code == 1
    assert env.teardown.await_count == 1


def test_run_full_exits_nonzero_for_blocked(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    env = AsyncMock()
    env.provision.return_value = _env_handle()
    env.execute.return_value = PhaseResult(
        outcome=Outcome.BLOCKED,
        signal="blocked",
        exit_code=0,
        stdout="blocked",
        duration_secs=1,
        preflight_passed=True,
        retries=0,
    )
    env.ssh_defaults = SSHConfig(host="", user="root", key_path="~/.ssh/id_rsa", port=22)

    monkeypatch.setattr("worker_manager.run_cli._load_config", lambda: config)
    monkeypatch.setattr("worker_manager.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "worker_manager.run_cli._resolve_agent_tool",
        lambda config, phase: AgentTool(cli=Cli.CLAUDE),
    )

    result = CliRunner().invoke(
        run,
        [
            "full",
            "--project",
            "proj",
            "--environment-profile",
            "default",
            "--branch",
            "main",
            "--spec-path",
            "tanren/specs/s1",
            "--phase",
            "do-task",
        ],
    )

    assert result.exit_code == 1
    assert env.teardown.await_count == 1


def test_run_execute_rejects_legacy_handle_schema(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    path = Path(config.data_dir) / "run-handles"
    path.mkdir(parents=True, exist_ok=True)
    legacy = {
        "env_id": "env-legacy",
        "vm_id": "vm-legacy",
        "project": "proj",
        "branch": "main",
        "workflow_id": "run-proj-legacy",
        "environment_profile": "default",
        "local_worktree_path": str(Path(config.github_dir) / "proj"),
        "workspace_path": "/workspace/proj",
        "teardown_commands": ["make clean"],
        "provision_start": time.monotonic(),
        "vm_handle": {
            "vm_id": "vm-legacy",
            "host": "203.0.113.10",
            "provider": "hetzner",
            "created_at": "2026-01-01T00:00:00Z",
            "labels": {},
            "hourly_cost": 0.5,
        },
        "ssh_defaults": {
            "user": "root",
            "key_path": "~/.ssh/id_rsa",
            "port": 22,
            "connect_timeout": 10,
        },
    }
    (path / "env-legacy.json").write_text(json.dumps(legacy))

    env = AsyncMock()
    monkeypatch.setattr("worker_manager.run_cli._load_config", lambda: config)
    monkeypatch.setattr("worker_manager.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "worker_manager.run_cli._resolve_agent_tool",
        lambda config, phase: AgentTool(cli=Cli.CLAUDE),
    )

    result = CliRunner().invoke(
        run,
        [
            "execute",
            "--handle",
            "env-legacy",
            "--project",
            "proj",
            "--spec-path",
            "tanren/specs/s1",
            "--phase",
            "do-task",
        ],
    )

    assert result.exit_code == 1
    assert "Run handle schema has changed" in result.output
