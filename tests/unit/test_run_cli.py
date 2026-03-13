"""Tests for tanren run CLI commands."""

from __future__ import annotations

import json
import time
from datetime import UTC, datetime
from pathlib import Path
from unittest.mock import AsyncMock

import pytest
import typer
from typer.testing import CliRunner

from tanren_cli.run_cli import (
    PersistedRunHandle,
    PersistedSSHDefaults,
    _load_handle,  # noqa: PLC2701
    _save_handle,  # noqa: PLC2701
    run,
)
from tanren_core.adapters.remote_types import VMHandle, VMProvider, WorkspacePath
from tanren_core.adapters.ssh import SSHConfig
from tanren_core.adapters.types import EnvironmentHandle, PhaseResult, RemoteEnvironmentRuntime
from tanren_core.config import Config
from tanren_core.roles import AgentTool
from tanren_core.schemas import Cli, Outcome


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


def _write_tanren_yml(config: Config, content: str) -> None:
    project_dir = Path(config.github_dir) / "proj"
    project_dir.mkdir(parents=True, exist_ok=True)
    (project_dir / "tanren.yml").write_text(content)


def test_run_provision_prints_and_saves_handle(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    env = AsyncMock()
    env.provision.return_value = _env_handle()
    env.ssh_defaults = SSHConfig(host="", user="root", key_path="~/.ssh/id_rsa", port=22)

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)

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

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "tanren_cli.run_cli._resolve_agent_tool",
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

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)

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

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "tanren_cli.run_cli._resolve_agent_tool",
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

    # Provision-time SSH connection must be closed
    runtime_conn = env.provision.return_value.runtime.connection
    runtime_conn.close.assert_awaited_once()


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

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "tanren_cli.run_cli._resolve_agent_tool",
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

    # Provision-time SSH connection must be closed
    runtime_conn = env.provision.return_value.runtime.connection
    runtime_conn.close.assert_awaited_once()


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

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "tanren_cli.run_cli._resolve_agent_tool",
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

    # Provision-time SSH connection must be closed
    runtime_conn = env.provision.return_value.runtime.connection
    runtime_conn.close.assert_awaited_once()


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
    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "tanren_cli.run_cli._resolve_agent_tool",
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


def test_run_execute_gate_uses_profile_gate_cmd_when_missing_flag(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    _persisted(config)
    _write_tanren_yml(
        config,
        "environment:\n  default:\n    type: remote\n    gate_cmd: make integration-check\n",
    )
    env = AsyncMock()
    env.execute.return_value = PhaseResult(
        outcome=Outcome.SUCCESS,
        signal="ok",
        exit_code=0,
        stdout="gate ok",
        duration_secs=1,
        preflight_passed=True,
        retries=0,
    )

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)

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
            "gate",
        ],
    )

    assert result.exit_code == 0
    dispatch = env.execute.await_args.args[1]
    assert dispatch.cli == Cli.BASH
    assert dispatch.gate_cmd == "make integration-check"


def test_run_execute_gate_rejects_blank_gate_cmd(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    _persisted(config)
    env = AsyncMock()

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)

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
            "gate",
            "--gate-cmd",
            "   ",
        ],
    )

    assert result.exit_code == 1
    assert "requires a non-empty gate command" in result.output
    env.execute.assert_not_called()


def test_load_handle_accepts_file_path(tmp_path: Path):
    config = _config(tmp_path)
    persisted = PersistedRunHandle(
        env_id="env-fp",
        vm_id="vm-fp",
        project="proj",
        branch="main",
        workflow_id="run-proj-fp",
        environment_profile="default",
        local_worktree_path=str(Path(config.github_dir) / "proj"),
        workspace_path="/workspace/proj",
        teardown_commands=(),
        provisioned_at_utc=datetime.now(UTC).isoformat(),
        vm_handle=VMHandle(
            vm_id="vm-fp",
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
    handle_file = tmp_path / "custom-handle.json"
    handle_file.write_text(persisted.model_dump_json(indent=2))

    loaded, loaded_path = _load_handle(config, str(handle_file))

    assert loaded.env_id == "env-fp"
    assert loaded.vm_id == "vm-fp"
    assert loaded_path == handle_file


def test_run_full_teardown_runs_even_when_handle_save_fails(tmp_path: Path, monkeypatch):
    config = _config(tmp_path)
    env = AsyncMock()
    env.provision.return_value = _env_handle()
    env.ssh_defaults = SSHConfig(host="", user="root", key_path="~/.ssh/id_rsa", port=22)

    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)
    monkeypatch.setattr(
        "tanren_cli.run_cli._save_handle",
        lambda _config, _persisted: (_ for _ in ()).throw(OSError("disk full")),
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
    assert env.execute.await_count == 0
    assert env.teardown.await_count == 1


def test_teardown_retains_external_handle_file(tmp_path: Path, monkeypatch):
    """Handle files outside run-handles/ are not deleted on teardown."""
    config = _config(tmp_path)
    persisted = PersistedRunHandle(
        env_id="env-ext",
        vm_id="vm-ext",
        project="proj",
        branch="main",
        workflow_id="run-proj-ext",
        environment_profile="default",
        local_worktree_path=str(Path(config.github_dir) / "proj"),
        workspace_path="/workspace/proj",
        teardown_commands=(),
        provisioned_at_utc=datetime.now(UTC).isoformat(),
        vm_handle=VMHandle(
            vm_id="vm-ext",
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
    external_file = tmp_path / "external-handle.json"
    external_file.write_text(persisted.model_dump_json(indent=2))

    env = AsyncMock()
    monkeypatch.setattr("tanren_cli.run_cli._load_config", lambda: config)
    monkeypatch.setattr("tanren_cli.run_cli._build_remote_execution_env", lambda cfg: env)

    result = CliRunner().invoke(run, ["teardown", "--handle", str(external_file)])

    assert result.exit_code == 0
    assert external_file.exists(), "external handle file should NOT be deleted"
    assert "handle_retained" in result.output


def test_load_handle_registry_not_shadowed_by_cwd_file(tmp_path: Path, monkeypatch):
    """A file named like an env_id in cwd must not shadow the registry."""
    config = _config(tmp_path)
    _persisted(config)  # creates env-123.json in registry

    # Create a decoy file named "env-123" in cwd (not valid JSON)
    monkeypatch.chdir(tmp_path)
    decoy = tmp_path / "env-123"
    decoy.write_text("not a handle")

    loaded, loaded_path = _load_handle(config, "env-123")

    assert loaded.env_id == "env-123"
    # Must resolve from registry, not from cwd decoy
    assert "run-handles" in str(loaded_path)


def test_persisted_handle_roundtrips_host_key_policy(tmp_path: Path):
    """host_key_policy survives save → load roundtrip."""
    config = _config(tmp_path)
    persisted = PersistedRunHandle(
        env_id="env-hkp",
        vm_id="vm-hkp",
        project="proj",
        branch="main",
        workflow_id="run-proj-hkp",
        environment_profile="default",
        local_worktree_path=str(Path(config.github_dir) / "proj"),
        workspace_path="/workspace/proj",
        teardown_commands=(),
        provisioned_at_utc=datetime.now(UTC).isoformat(),
        vm_handle=VMHandle(
            vm_id="vm-hkp",
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
            host_key_policy="reject",
        ),
    )

    _save_handle(config, persisted)

    loaded, _ = _load_handle(config, "env-hkp")
    assert loaded.ssh_defaults.host_key_policy == "reject"


def test_persisted_handle_rejects_invalid_host_key_policy(tmp_path: Path, capsys):
    """Invalid host_key_policy values are rejected at load time."""
    config = _config(tmp_path)
    path = Path(config.data_dir) / "run-handles"
    path.mkdir(parents=True, exist_ok=True)
    handle_data = {
        "env_id": "env-bad-hkp",
        "vm_id": "vm-bad-hkp",
        "project": "proj",
        "branch": "main",
        "workflow_id": "run-proj-bad-hkp",
        "environment_profile": "default",
        "local_worktree_path": str(Path(config.github_dir) / "proj"),
        "workspace_path": "/workspace/proj",
        "teardown_commands": [],
        "provisioned_at_utc": datetime.now(UTC).isoformat(),
        "vm_handle": {
            "vm_id": "vm-bad-hkp",
            "host": "203.0.113.10",
            "provider": "hetzner",
            "created_at": "2026-01-01T00:00:00Z",
            "hourly_cost": 0.5,
        },
        "ssh_defaults": {
            "user": "root",
            "key_path": "~/.ssh/id_rsa",
            "port": 22,
            "connect_timeout": 10,
            "host_key_policy": "bogus",
        },
    }
    (path / "env-bad-hkp.json").write_text(json.dumps(handle_data))

    with pytest.raises(typer.Exit) as exc_info:
        _load_handle(config, "env-bad-hkp")

    assert exc_info.value.exit_code == 1
    captured = capsys.readouterr()
    assert "Run handle schema has changed" in captured.err
