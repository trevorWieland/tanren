"""Tests for manager module."""

import os
from pathlib import Path
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.remote_types import VMAssignment, VMProvider
from tanren_core.config import Config
from tanren_core.manager import (
    _GATE_OUTPUT_LINES_FAIL,  # noqa: PLC2701
    _GATE_OUTPUT_LINES_SUCCESS,  # noqa: PLC2701
    _TAIL_OUTPUT_LINES,  # noqa: PLC2701
    WorkerManager,
    _build_gate_output,  # noqa: PLC2701
    build_tail_output,
)
from tanren_core.schemas import Outcome


class TestWorkerManagerInit:
    def test_creates_with_config(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            roles_config_path=str(tmp_path / "roles.yml"),
        )
        manager = WorkerManager(config)
        assert manager._config == config

    def test_directories_derived_from_config(self, tmp_path: Path):
        ipc = tmp_path / "ipc"
        config = Config(
            ipc_dir=str(ipc),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            roles_config_path=str(tmp_path / "roles.yml"),
        )
        manager = WorkerManager(config)
        assert manager._dispatch_dir == ipc / "dispatch"
        assert manager._results_dir == ipc / "results"
        assert manager._in_progress_dir == ipc / "in-progress"
        assert manager._input_dir == ipc / "input"

    def test_get_execution_environment_returns_configured_env(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            roles_config_path=str(tmp_path / "roles.yml"),
        )
        manager = WorkerManager(config)
        assert manager.get_execution_environment() is manager._execution_env

    def test_remote_env_autoloads_before_git_token_read(self, tmp_path: Path, monkeypatch):
        remote_cfg = tmp_path / "remote.yml"
        remote_cfg.write_text(
            "git:\n"
            "  auth: token\n"
            "  token_env: CUSTOM_GIT_TOKEN\n"
            "provisioner:\n"
            "  type: manual\n"
            "  settings:\n"
            "    vms:\n"
            "      - vm_id: vm-1\n"
            "        host: 10.0.0.1\n"
            "secrets:\n"
            "  developer_secrets_path: /tmp/unused.env\n"
            "repos:\n"
            "  demo: https://github.com/org/demo.git\n"
        )

        roles_yml = tmp_path / "roles.yml"
        roles_yml.write_text(
            "agents:\n  default:\n    cli: claude\n    auth: subscription\n    model: opus\n"
        )

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            remote_config_path=str(remote_cfg),
            roles_config_path=str(roles_yml),
        )

        monkeypatch.delenv("CUSTOM_GIT_TOKEN", raising=False)

        seen: dict[str, object] = {}

        class _FakeSecretLoader:
            def __init__(self, config, *, required_clis=None):
                seen["secret_config"] = config

            def autoload_into_env(self, *, override: bool = False) -> None:
                seen["autoload_override"] = override
                os.environ["CUSTOM_GIT_TOKEN"] = "from-loader"

        class _FakeGitWorkspaceManager:
            def __init__(self, git_auth):
                seen["git_token"] = git_auth.token

        class _FakeSSHExecutionEnvironment:
            def __init__(self, **kwargs):
                seen["secret_loader"] = kwargs["secret_loader"]
                seen["repo_urls"] = kwargs["repo_urls"]

        monkeypatch.setattr("tanren_core.builder.SecretLoader", _FakeSecretLoader)
        monkeypatch.setattr(
            "tanren_core.builder.GitWorkspaceManager",
            _FakeGitWorkspaceManager,
        )
        monkeypatch.setattr(
            "tanren_core.builder.SSHExecutionEnvironment",
            _FakeSSHExecutionEnvironment,
        )
        monkeypatch.setattr(
            "tanren_core.builder.SqliteVMStateStore",
            lambda _: object(),
        )
        monkeypatch.setattr(
            "tanren_core.builder.ManualVMProvisioner",
            lambda _vms, _store: object(),
        )
        monkeypatch.setattr(
            "tanren_core.builder.UbuntuBootstrapper",
            lambda *args, **kwargs: object(),
        )
        monkeypatch.setattr(
            "tanren_core.builder.RemoteAgentRunner",
            lambda **kwargs: object(),
        )

        manager = WorkerManager(config=config, emitter=NullEventEmitter())
        assert manager.get_execution_environment() is not None
        assert seen["autoload_override"] is False
        assert seen["git_token"] == "from-loader"


class TestBuildGateOutput:
    def test_none_when_stdout_is_none(self):
        assert _build_gate_output(None, Outcome.SUCCESS) is None

    def test_none_when_stdout_is_empty(self):
        assert _build_gate_output("", Outcome.FAIL) is None

    def test_success_truncates_to_100_lines(self):
        lines = [f"line {i}" for i in range(200)]
        result = _build_gate_output("\n".join(lines), Outcome.SUCCESS)
        assert result is not None
        result_lines = result.split("\n")
        assert len(result_lines) == _GATE_OUTPUT_LINES_SUCCESS
        assert result_lines[0] == "line 100"
        assert result_lines[-1] == "line 199"

    def test_fail_truncates_to_300_lines(self):
        lines = [f"line {i}" for i in range(500)]
        result = _build_gate_output("\n".join(lines), Outcome.FAIL)
        assert result is not None
        result_lines = result.split("\n")
        assert len(result_lines) == _GATE_OUTPUT_LINES_FAIL
        assert result_lines[0] == "line 200"
        assert result_lines[-1] == "line 499"

    def test_short_output_returned_intact(self):
        result = _build_gate_output("hello\nworld", Outcome.SUCCESS)
        assert result == "hello\nworld"

    def test_error_uses_fail_limit(self):
        lines = [f"line {i}" for i in range(500)]
        result = _build_gate_output("\n".join(lines), Outcome.ERROR)
        assert result is not None
        result_lines = result.split("\n")
        assert len(result_lines) == _GATE_OUTPUT_LINES_FAIL


class TestBuildTailOutput:
    def test_none_when_stdout_is_none(self):
        assert build_tail_output(None) is None

    def test_none_when_stdout_is_empty(self):
        assert build_tail_output("") is None

    def test_truncates_to_200_lines(self):
        lines = [f"line {i}" for i in range(400)]
        result = build_tail_output("\n".join(lines))
        assert result is not None
        result_lines = result.split("\n")
        assert len(result_lines) == _TAIL_OUTPUT_LINES
        assert result_lines[0] == "line 200"
        assert result_lines[-1] == "line 399"

    def test_short_output_returned_intact(self):
        result = build_tail_output("one\ntwo\nthree")
        assert result == "one\ntwo\nthree"


class TestRecoverVmState:
    @pytest.mark.asyncio
    async def test_releases_all_stale_assignments_via_provider(self, tmp_path: Path, monkeypatch):
        remote_cfg = tmp_path / "remote.yml"
        remote_cfg.write_text(
            "provisioner:\n"
            "  type: manual\n"
            "  settings:\n"
            "    vms:\n"
            "      - vm_id: vm-1\n"
            "        host: 10.0.0.1\n"
        )
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            remote_config_path=str(remote_cfg),
            roles_config_path=str(tmp_path / "roles.yml"),
        )
        assignment = VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="spec",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00Z",
        )
        store = AsyncMock()
        store.get_active_assignments.return_value = [assignment]
        provisioner = AsyncMock()

        monkeypatch.setattr(
            "tanren_core.manager.SqliteVMStateStore",
            lambda _path: store,
        )
        monkeypatch.setattr(
            "tanren_core.manager.ManualVMProvisioner",
            lambda _vms, _store: provisioner,
        )

        manager = WorkerManager(
            config=config,
            execution_env=AsyncMock(),
            emitter=NullEventEmitter(),
        )
        await manager._recover_vm_state()

        assert provisioner.release.await_count == 1
        handle = provisioner.release.await_args.args[0]
        assert handle.vm_id == "vm-1"
        assert handle.host == "10.0.0.1"
        assert handle.provider == VMProvider.MANUAL
        store.record_release.assert_awaited_once_with("vm-1")
        store.close.assert_awaited_once()

    @pytest.mark.asyncio
    async def test_records_release_even_if_provider_release_fails(
        self, tmp_path: Path, monkeypatch
    ):
        remote_cfg = tmp_path / "remote.yml"
        remote_cfg.write_text(
            "provisioner:\n"
            "  type: manual\n"
            "  settings:\n"
            "    vms:\n"
            "      - vm_id: vm-1\n"
            "        host: 10.0.0.1\n"
        )
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            remote_config_path=str(remote_cfg),
            roles_config_path=str(tmp_path / "roles.yml"),
        )
        assignment = VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="spec",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00Z",
        )
        store = AsyncMock()
        store.get_active_assignments.return_value = [assignment]
        provisioner = AsyncMock()
        provisioner.release.side_effect = RuntimeError("release failed")

        monkeypatch.setattr(
            "tanren_core.manager.SqliteVMStateStore",
            lambda _path: store,
        )
        monkeypatch.setattr(
            "tanren_core.manager.ManualVMProvisioner",
            lambda _vms, _store: provisioner,
        )

        manager = WorkerManager(
            config=config,
            execution_env=AsyncMock(),
            emitter=NullEventEmitter(),
        )
        await manager._recover_vm_state()

        store.record_release.assert_awaited_once_with("vm-1")
