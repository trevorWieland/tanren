"""Tests for LocalExecutionEnvironment."""

from pathlib import Path
from unittest.mock import AsyncMock, patch

import pytest

from worker_manager.adapters.local_environment import LocalExecutionEnvironment
from worker_manager.adapters.types import (
    AccessInfo,
    EnvironmentHandle,
    LocalEnvironmentRuntime,
    PhaseResult,
    ProvisionError,
)
from worker_manager.config import Config
from worker_manager.env.validator import EnvReport
from worker_manager.postflight import PostflightResult
from worker_manager.preflight import PreflightResult
from worker_manager.process import ProcessResult
from worker_manager.schemas import Cli, Dispatch, Outcome, Phase


def _make_config(tmp_path: Path) -> Config:
    return Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
    )


def _make_dispatch(phase: Phase = Phase.DO_TASK, cli: Cli = Cli.OPENCODE) -> Dispatch:
    return Dispatch(
        workflow_id="wf-test-1-1000",
        phase=phase,
        project="test",
        spec_folder="tanren/specs/test",
        branch="test-branch",
        cli=cli,
        model="test-model",
        gate_cmd="make check" if cli == Cli.BASH else None,
        context=None,
        timeout=300,
    )


def _make_env(tmp_path: Path):
    """Create mocked adapters and LocalExecutionEnvironment."""
    # Create worktree directory structure
    wt_path = tmp_path / "test-wt-1"
    wt_path.mkdir(parents=True, exist_ok=True)
    spec_path = wt_path / "tanren" / "specs" / "test"
    spec_path.mkdir(parents=True, exist_ok=True)

    # Mock adapters
    env_validator = AsyncMock()
    preflight = AsyncMock()
    postflight = AsyncMock()
    spawner = AsyncMock()
    heartbeat = AsyncMock()
    config = _make_config(tmp_path)

    env = LocalExecutionEnvironment(
        env_validator=env_validator,
        preflight=preflight,
        postflight=postflight,
        spawner=spawner,
        heartbeat=heartbeat,
        config=config,
    )

    return env, env_validator, preflight, postflight, spawner, heartbeat, config


class TestProvision:
    @pytest.mark.asyncio
    async def test_provision_success(self, tmp_path: Path):
        env, env_validator, preflight, *_ = _make_env(tmp_path)
        config = _make_config(tmp_path)
        dispatch = _make_dispatch()

        env_validator.load_and_validate.return_value = (
            EnvReport(passed=True),
            {"KEY": "val"},
        )
        preflight.run.return_value = PreflightResult(
            passed=True,
            file_hashes={"spec.md": "abc123"},
            file_backups={"spec.md": "# Spec"},
        )

        handle = await env.provision(dispatch, config)

        assert isinstance(handle, EnvironmentHandle)
        assert handle.worktree_path == tmp_path / "test-wt-1"
        assert handle.branch == "test-branch"
        assert handle.project == "test"
        assert handle.runtime.kind == "local"
        assert handle.runtime.task_env == {"KEY": "val"}
        assert handle.runtime.preflight_result is not None
        assert handle.runtime.preflight_result.passed is True
        assert handle.runtime.env_report is not None
        assert handle.runtime.env_report.passed is True

        env_validator.load_and_validate.assert_awaited_once_with(tmp_path / "test-wt-1")
        preflight.run.assert_awaited_once()

    @pytest.mark.asyncio
    async def test_provision_env_failure(self, tmp_path: Path):
        env, env_validator, preflight, *_ = _make_env(tmp_path)
        config = _make_config(tmp_path)
        dispatch = _make_dispatch()

        env_validator.load_and_validate.return_value = (
            EnvReport(passed=False),
            {},
        )

        with pytest.raises(ProvisionError) as exc_info:
            await env.provision(dispatch, config)

        assert exc_info.value.result.outcome == Outcome.ERROR
        assert exc_info.value.result.exit_code == -1
        assert exc_info.value.preflight_result is None

        # Preflight should NOT have been called
        preflight.run.assert_not_awaited()

    @pytest.mark.asyncio
    async def test_provision_preflight_failure(self, tmp_path: Path):
        env, env_validator, preflight, *_ = _make_env(tmp_path)
        config = _make_config(tmp_path)
        dispatch = _make_dispatch()

        env_validator.load_and_validate.return_value = (
            EnvReport(passed=True),
            {},
        )
        preflight.run.return_value = PreflightResult(
            passed=False,
            error="Command file missing: .claude/commands/tanren/do-task.md",
        )

        with pytest.raises(ProvisionError) as exc_info:
            await env.provision(dispatch, config)

        assert exc_info.value.result.outcome == Outcome.ERROR
        expected = "Command file missing: .claude/commands/tanren/do-task.md"
        assert exc_info.value.result.tail_output == expected
        assert exc_info.value.preflight_result is not None
        assert exc_info.value.preflight_result.passed is False


class TestExecute:
    def _make_handle(self, tmp_path: Path) -> EnvironmentHandle:
        wt_path = tmp_path / "test-wt-1"
        wt_path.mkdir(parents=True, exist_ok=True)
        spec_path = wt_path / "tanren" / "specs" / "test"
        spec_path.mkdir(parents=True, exist_ok=True)
        return EnvironmentHandle(
            env_id="test-env-id",
            worktree_path=wt_path,
            branch="test-branch",
            project="test",
            runtime=LocalEnvironmentRuntime(
                preflight_result=PreflightResult(
                    passed=True,
                    file_hashes={"spec.md": "abc123"},
                    file_backups={"spec.md": "# Spec"},
                ),
                task_env={"KEY": "val"},
                env_report=EnvReport(passed=True),
            ),
        )

    @pytest.mark.asyncio
    @patch("worker_manager.adapters.local_environment.compute_plan_hash", new_callable=AsyncMock)
    @patch(
        "worker_manager.adapters.local_environment.count_unchecked_tasks",
        new_callable=AsyncMock,
    )
    @patch("worker_manager.adapters.local_environment.map_outcome")
    @patch("worker_manager.adapters.local_environment.extract_signal")
    async def test_execute_success(
        self,
        mock_extract_signal,
        mock_map_outcome,
        mock_count_unchecked,
        mock_plan_hash,
        tmp_path: Path,
    ):
        env, _, _, postflight, spawner, _heartbeat, config = _make_env(tmp_path)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        handle = self._make_handle(tmp_path)

        spawner.spawn.return_value = ProcessResult(
            exit_code=0,
            stdout="do-task-status: complete",
            timed_out=False,
            duration_secs=10,
        )
        mock_extract_signal.return_value = "complete"
        mock_map_outcome.return_value = (Outcome.SUCCESS, "complete")
        mock_count_unchecked.return_value = 0
        mock_plan_hash.return_value = "abcd1234"
        postflight.run.return_value = PostflightResult(pushed=True)

        result = await env.execute(handle, dispatch, config, dispatch_stem="test-stem")

        assert isinstance(result, PhaseResult)
        assert result.outcome == Outcome.SUCCESS
        assert result.signal == "complete"
        assert result.exit_code == 0
        assert result.preflight_passed is True
        assert result.unchecked_tasks == 0
        assert result.plan_hash == "abcd1234"
        assert result.retries == 0

        _heartbeat.start.assert_awaited_once_with("test-stem")
        _heartbeat.stop.assert_awaited_once_with("test-stem")
        spawner.spawn.assert_awaited_once()

    @pytest.mark.asyncio
    @patch("worker_manager.adapters.local_environment.compute_plan_hash", new_callable=AsyncMock)
    @patch(
        "worker_manager.adapters.local_environment.count_unchecked_tasks",
        new_callable=AsyncMock,
    )
    @patch("worker_manager.adapters.local_environment.map_outcome")
    @patch("worker_manager.adapters.local_environment.extract_signal")
    async def test_execute_postflight_runs_for_push_phases(
        self,
        mock_extract_signal,
        mock_map_outcome,
        mock_count_unchecked,
        mock_plan_hash,
        tmp_path: Path,
    ):
        env, _, _, postflight, spawner, _heartbeat, config = _make_env(tmp_path)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        handle = self._make_handle(tmp_path)

        spawner.spawn.return_value = ProcessResult(
            exit_code=0,
            stdout="do-task-status: complete",
            timed_out=False,
            duration_secs=10,
        )
        mock_extract_signal.return_value = "complete"
        mock_map_outcome.return_value = (Outcome.SUCCESS, "complete")
        mock_count_unchecked.return_value = 0
        mock_plan_hash.return_value = "abcd1234"
        postflight.run.return_value = PostflightResult(pushed=True)

        result = await env.execute(handle, dispatch, config, dispatch_stem="test-stem")

        # Postflight should be called for DO_TASK (a push phase)
        postflight.run.assert_awaited_once()
        assert result.postflight_result is not None
        assert result.postflight_result.pushed is True

        # Verify postflight was called with correct args
        call_args = postflight.run.call_args
        assert call_args[0][0] == handle.worktree_path  # worktree_path
        assert call_args[0][1] == "test-branch"  # branch
        assert call_args[0][2] == "do-task"  # phase value
        assert call_args[0][3] == {"spec.md": "abc123"}  # file_hashes
        assert call_args[0][4] == {"spec.md": "# Spec"}  # file_backups

    @pytest.mark.asyncio
    @patch("worker_manager.adapters.local_environment.compute_plan_hash", new_callable=AsyncMock)
    @patch(
        "worker_manager.adapters.local_environment.count_unchecked_tasks",
        new_callable=AsyncMock,
    )
    @patch("worker_manager.adapters.local_environment.map_outcome")
    @patch("worker_manager.adapters.local_environment.extract_signal")
    async def test_execute_postflight_skipped_for_gate(
        self,
        mock_extract_signal,
        mock_map_outcome,
        mock_count_unchecked,
        mock_plan_hash,
        tmp_path: Path,
    ):
        env, _, _, postflight, spawner, _heartbeat, config = _make_env(tmp_path)
        dispatch = _make_dispatch(phase=Phase.GATE, cli=Cli.BASH)
        handle = self._make_handle(tmp_path)

        spawner.spawn.return_value = ProcessResult(
            exit_code=0,
            stdout="All tests passed",
            timed_out=False,
            duration_secs=5,
        )
        mock_extract_signal.return_value = None
        mock_map_outcome.return_value = (Outcome.SUCCESS, None)
        mock_count_unchecked.return_value = 0
        mock_plan_hash.return_value = "abcd1234"

        result = await env.execute(handle, dispatch, config, dispatch_stem="test-stem")

        # Postflight should NOT be called for GATE phase
        postflight.run.assert_not_awaited()
        assert result.postflight_result is None
        assert result.outcome == Outcome.SUCCESS


class TestGetAccessInfo:
    @pytest.mark.asyncio
    async def test_get_access_info_local(self, tmp_path: Path):
        env, *_ = _make_env(tmp_path)
        wt_path = tmp_path / "test-wt-1"
        handle = EnvironmentHandle(
            env_id="test-env-id",
            worktree_path=wt_path,
            branch="test-branch",
            project="test",
            runtime=LocalEnvironmentRuntime(),
        )

        access_info = await env.get_access_info(handle)

        assert isinstance(access_info, AccessInfo)
        assert access_info.working_dir == str(wt_path)
        assert access_info.status == "local"
        assert access_info.ssh is None
        assert access_info.vscode is None


class TestTeardown:
    @pytest.mark.asyncio
    async def test_teardown_noop(self, tmp_path: Path):
        env, *_ = _make_env(tmp_path)
        wt_path = tmp_path / "test-wt-1"
        handle = EnvironmentHandle(
            env_id="test-env-id",
            worktree_path=wt_path,
            branch="test-branch",
            project="test",
            runtime=LocalEnvironmentRuntime(),
        )

        # Should complete without raising any errors
        await env.teardown(handle)
