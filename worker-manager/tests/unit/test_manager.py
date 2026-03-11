"""Tests for manager module."""

import os
from pathlib import Path

from worker_manager.adapters.null_emitter import NullEventEmitter
from worker_manager.config import Config
from worker_manager.manager import (
    _GATE_OUTPUT_LINES_FAIL,
    _GATE_OUTPUT_LINES_SUCCESS,
    _TAIL_OUTPUT_LINES,
    WorkerManager,
    _build_gate_output,
    _build_tail_output,
)
from worker_manager.schemas import Outcome


class TestWorkerManagerInit:
    def test_creates_with_config(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
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

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            remote_config_path=str(remote_cfg),
        )

        monkeypatch.delenv("CUSTOM_GIT_TOKEN", raising=False)

        seen: dict[str, object] = {}

        class _FakeSecretLoader:
            def __init__(self, config):
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

        monkeypatch.setattr("worker_manager.secrets.SecretLoader", _FakeSecretLoader)
        monkeypatch.setattr(
            "worker_manager.adapters.git_workspace.GitWorkspaceManager",
            _FakeGitWorkspaceManager,
        )
        monkeypatch.setattr(
            "worker_manager.adapters.ssh_environment.SSHExecutionEnvironment",
            _FakeSSHExecutionEnvironment,
        )
        monkeypatch.setattr(
            "worker_manager.adapters.sqlite_vm_state.SqliteVMStateStore",
            lambda _: object(),
        )
        monkeypatch.setattr(
            "worker_manager.adapters.manual_vm.ManualVMProvisioner",
            lambda _vms, _store: object(),
        )
        monkeypatch.setattr(
            "worker_manager.adapters.ubuntu_bootstrap.UbuntuBootstrapper",
            lambda *args, **kwargs: object(),
        )
        monkeypatch.setattr(
            "worker_manager.adapters.remote_runner.RemoteAgentRunner",
            lambda: object(),
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
        result_lines = result.split("\n")
        assert len(result_lines) == _GATE_OUTPUT_LINES_SUCCESS
        assert result_lines[0] == "line 100"
        assert result_lines[-1] == "line 199"

    def test_fail_truncates_to_300_lines(self):
        lines = [f"line {i}" for i in range(500)]
        result = _build_gate_output("\n".join(lines), Outcome.FAIL)
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
        result_lines = result.split("\n")
        assert len(result_lines) == _GATE_OUTPUT_LINES_FAIL


class TestBuildTailOutput:
    def test_none_when_stdout_is_none(self):
        assert _build_tail_output(None) is None

    def test_none_when_stdout_is_empty(self):
        assert _build_tail_output("") is None

    def test_truncates_to_200_lines(self):
        lines = [f"line {i}" for i in range(400)]
        result = _build_tail_output("\n".join(lines))
        result_lines = result.split("\n")
        assert len(result_lines) == _TAIL_OUTPUT_LINES
        assert result_lines[0] == "line 200"
        assert result_lines[-1] == "line 399"

    def test_short_output_returned_intact(self):
        result = _build_tail_output("one\ntwo\nthree")
        assert result == "one\ntwo\nthree"
