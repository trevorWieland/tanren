"""Verify each adapter delegates correctly to its underlying module function."""

from pathlib import Path
from unittest.mock import AsyncMock, patch

import pytest

from tanren_core.adapters.dotenv_provisioner import DotenvEnvProvisioner
from tanren_core.adapters.dotenv_validator import DotenvEnvValidator
from tanren_core.adapters.git_postflight import GitPostflightRunner
from tanren_core.adapters.git_preflight import GitPreflightRunner
from tanren_core.adapters.git_worktree import GitWorktreeManager
from tanren_core.adapters.subprocess_spawner import SubprocessSpawner
from tanren_core.config import Config
from tanren_core.env.validator import EnvReport
from tanren_core.postflight import PostflightResult
from tanren_core.preflight import PreflightResult
from tanren_core.process import ProcessResult
from tanren_core.schemas import Cli, Dispatch, Phase


class TestGitWorktreeManager:
    @pytest.mark.asyncio
    @patch("tanren_core.adapters.git_worktree.create_worktree", new_callable=AsyncMock)
    async def test_create_delegates(self, mock_create):
        mock_create.return_value = Path("/tmp/project-wt-1")
        mgr = GitWorktreeManager()
        result = await mgr.create("project", 1, "feat-1", "/home/github")
        mock_create.assert_awaited_once_with("project", 1, "feat-1", "/home/github")
        assert result == Path("/tmp/project-wt-1")

    @pytest.mark.asyncio
    @patch("tanren_core.adapters.git_worktree.register_worktree", new_callable=AsyncMock)
    async def test_register_delegates(self, mock_register):
        mgr = GitWorktreeManager()
        reg_path = Path("/tmp/worktrees.json")
        wt_path = Path("/tmp/project-wt-1")
        await mgr.register(reg_path, "wf-1", "project", 1, "feat-1", wt_path, "/home/github")
        mock_register.assert_awaited_once_with(
            reg_path, "wf-1", "project", 1, "feat-1", wt_path, "/home/github"
        )

    @pytest.mark.asyncio
    @patch("tanren_core.adapters.git_worktree.cleanup_worktree", new_callable=AsyncMock)
    async def test_cleanup_delegates(self, mock_cleanup):
        mgr = GitWorktreeManager()
        reg_path = Path("/tmp/worktrees.json")
        await mgr.cleanup("wf-1", reg_path, "/home/github")
        mock_cleanup.assert_awaited_once_with("wf-1", reg_path, "/home/github")


class TestGitPreflightRunner:
    @pytest.mark.asyncio
    @patch("tanren_core.adapters.git_preflight.run_preflight", new_callable=AsyncMock)
    async def test_run_delegates(self, mock_run):
        mock_run.return_value = PreflightResult(passed=True)
        runner = GitPreflightRunner()
        result = await runner.run(Path("/tmp/wt"), "feat-1", Path("/tmp/spec"), "do-task")
        mock_run.assert_awaited_once_with(Path("/tmp/wt"), "feat-1", Path("/tmp/spec"), "do-task")
        assert result.passed is True


class TestGitPostflightRunner:
    @pytest.mark.asyncio
    @patch("tanren_core.adapters.git_postflight.run_postflight", new_callable=AsyncMock)
    async def test_run_delegates(self, mock_run):
        mock_run.return_value = PostflightResult()
        runner = GitPostflightRunner()
        hashes = {"spec.md": "abc123"}
        backups = {"spec.md": "# Spec"}
        result = await runner.run(
            Path("/tmp/wt"), "feat-1", "do-task", hashes, backups, skip_push=True
        )
        mock_run.assert_awaited_once_with(
            Path("/tmp/wt"), "feat-1", "do-task", hashes, backups, skip_push=True
        )
        assert result.pushed is False


class TestSubprocessSpawner:
    @pytest.mark.asyncio
    @patch("tanren_core.adapters.subprocess_spawner.spawn_process", new_callable=AsyncMock)
    async def test_spawn_delegates(self, mock_spawn):
        mock_spawn.return_value = ProcessResult(
            exit_code=0, stdout="ok", timed_out=False, duration_secs=5
        )
        spawner = SubprocessSpawner()
        dispatch = Dispatch(
            workflow_id="wf-test-1-1000",
            phase=Phase.GATE,
            project="test",
            spec_folder="specs/s0001",
            branch="feat-1",
            cli=Cli.BASH,
            model=None,
            gate_cmd="make check",
            context=None,
            timeout=60,
        )
        config = Config(
            ipc_dir="/tmp/ipc",
            github_dir="/tmp/github",
            data_dir="/tmp/data",
            worktree_registry_path="/tmp/worktrees.json",
        )
        env = {"FOO": "bar"}
        result = await spawner.spawn(dispatch, Path("/tmp/wt"), config, task_env=env)
        mock_spawn.assert_awaited_once_with(dispatch, Path("/tmp/wt"), config, task_env=env)
        assert result.exit_code == 0


class TestDotenvEnvValidator:
    @pytest.mark.asyncio
    @patch("tanren_core.adapters.dotenv_validator.load_and_validate_env", new_callable=AsyncMock)
    async def test_load_and_validate_delegates(self, mock_validate):
        mock_validate.return_value = (EnvReport(passed=True), {"KEY": "val"})
        validator = DotenvEnvValidator()
        report, env = await validator.load_and_validate(Path("/tmp/project"))
        mock_validate.assert_awaited_once_with(Path("/tmp/project"))
        assert report.passed is True
        assert env == {"KEY": "val"}


class TestDotenvEnvProvisioner:
    @patch("tanren_core.adapters.dotenv_provisioner.provision_worktree_env")
    def test_provision_delegates(self, mock_provision):
        mock_provision.return_value = 3
        provisioner = DotenvEnvProvisioner()
        count = provisioner.provision(Path("/tmp/wt"), Path("/tmp/project"))
        mock_provision.assert_called_once_with(Path("/tmp/wt"), Path("/tmp/project"))
        assert count == 3
