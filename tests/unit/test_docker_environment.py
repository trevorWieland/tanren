"""Tests for DockerExecutionEnvironment composition layer."""

from __future__ import annotations

import asyncio
import time
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from tanren_core.adapters.docker_connection import DockerConfig
from tanren_core.adapters.docker_environment import DockerExecutionEnvironment
from tanren_core.adapters.remote_types import (
    DryRunInfo,
    RemoteAgentResult,
    RemoteResult,
    SecretBundle,
    VMAssignment,
    VMProvider,
    VMRequirements,
    WorkspacePath,
)
from tanren_core.adapters.types import (
    AccessInfo,
    DockerEnvironmentRuntime,
    EnvironmentHandle,
    PhaseResult,
)
from tanren_core.env.environment_schema import (
    DockerExecutionConfig,
    EnvironmentProfile,
    EnvironmentProfileType,
    McpServerConfig,
    ResourceRequirements,
)
from tanren_core.errors import ErrorClass
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase
from tanren_core.worker_config import WorkerConfig

_DOCKER_ENV = "tanren_core.adapters.docker_environment"

DEFAULT_PROFILE = EnvironmentProfile(name="default")

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_config(tmp_path: Path) -> WorkerConfig:
    return WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path),
        data_dir=str(tmp_path / "data"),
        db_url=str(tmp_path / "events.db"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        roles_config_path=str(tmp_path / "roles.yml"),
    )


def _make_dispatch(
    phase: Phase = Phase.DO_TASK,
    cli: Cli = Cli.CLAUDE,
    project: str = "test-project",
    branch: str = "main",
    context: str = "Do the work",
    resolved_profile: EnvironmentProfile | None = None,
    project_env: dict[str, str] | None = None,
    cloud_secrets: dict[str, str] | None = None,
    required_secrets: tuple[str, ...] = (),
) -> Dispatch:
    return Dispatch(
        workflow_id="wf-test-project-42-1000",
        phase=phase,
        project=project,
        spec_folder="tanren/specs/feature",
        branch=branch,
        cli=cli,
        model="sonnet",
        gate_cmd="make check" if cli == Cli.BASH else None,
        context=context,
        timeout=300,
        environment_profile="default",
        resolved_profile=resolved_profile or _make_docker_profile(),
        project_env=project_env or {},
        cloud_secrets=cloud_secrets or {},
        required_secrets=required_secrets,
    )


def _make_docker_profile(
    *,
    setup: tuple[str, ...] = ("make setup",),
    teardown: tuple[str, ...] = ("make clean",),
    mcp: dict[str, McpServerConfig] | None = None,
    resources: ResourceRequirements | None = None,
    docker_config: DockerExecutionConfig | None = None,
) -> EnvironmentProfile:
    return EnvironmentProfile(
        name="default",
        type=EnvironmentProfileType.DOCKER,
        resources=resources or ResourceRequirements(cpu=2, memory_gb=4, gpu=False),
        setup=setup,
        teardown=teardown,
        mcp=mcp or {},
        docker_config=docker_config
        or DockerExecutionConfig(
            image="ubuntu:24.04",
            repo_url="https://github.com/test/repo.git",
        ),
    )


def _make_workspace() -> WorkspacePath:
    return WorkspacePath(path="/workspace/test-project", project="test-project", branch="main")


_OK_REMOTE_RESULT = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)


def _make_agent_result(
    exit_code: int = 0,
    stdout: str = "done",
    stderr: str = "",
    timed_out: bool = False,
    signal_content: str = "success",
) -> RemoteAgentResult:
    return RemoteAgentResult(
        exit_code=exit_code,
        stdout=stdout,
        timed_out=timed_out,
        duration_secs=30,
        stderr=stderr,
        signal_content=signal_content,
    )


def _make_handle(
    conn=None, workspace=None, *, teardown_commands=("make clean",)
) -> EnvironmentHandle:
    """Build a minimal EnvironmentHandle for execute/teardown tests."""
    ws = workspace or _make_workspace()
    return EnvironmentHandle(
        env_id="env-test-1",
        worktree_path=Path(ws.path),
        branch="main",
        project="test-project",
        runtime=DockerEnvironmentRuntime(
            container_id="abc123container",
            connection=conn or AsyncMock(),
            workspace_path=ws,
            profile=_make_docker_profile(),
            teardown_commands=teardown_commands,
            provision_start=time.monotonic(),
            workflow_id="wf-test-project-42-1000",
            docker_socket_url=None,
        ),
    )


# ---------------------------------------------------------------------------
# Fixture
# ---------------------------------------------------------------------------


@pytest.fixture
def docker_env_kit(tmp_path: Path):
    """Build a DockerExecutionEnvironment with all sub-adapters mocked."""
    bootstrapper = AsyncMock()
    workspace_mgr = AsyncMock()
    runner = AsyncMock()
    state_store = AsyncMock()
    secret_loader = MagicMock()

    # Configure workspace_mgr return values
    workspace_mgr.setup.return_value = _make_workspace()
    workspace_mgr.inject_secrets.return_value = None
    workspace_mgr.inject_mcp_config = AsyncMock()
    workspace_mgr.cleanup.return_value = None
    workspace_mgr.push_command = MagicMock(return_value="git push origin main")

    # Configure runner return value
    runner.run.return_value = _make_agent_result()

    # Configure secret_loader
    secret_loader.build_bundle.return_value = SecretBundle()

    # Configure state_store
    state_store.record_assignment.return_value = None
    state_store.record_release.return_value = None
    state_store.get_active_assignments.return_value = []
    state_store.close.return_value = None

    docker_config = DockerConfig(image="ubuntu:24.04")

    env = DockerExecutionEnvironment(
        bootstrapper=bootstrapper,
        workspace_mgr=workspace_mgr,
        runner=runner,
        state_store=state_store,
        secret_loader=secret_loader,
        docker_config=docker_config,
        repo_urls={"test-project": "https://github.com/test/repo.git"},
        agent_user="tanren",
    )

    return {
        "env": env,
        "bootstrapper": bootstrapper,
        "workspace_mgr": workspace_mgr,
        "runner": runner,
        "state_store": state_store,
        "secret_loader": secret_loader,
        "docker_config": docker_config,
        "config": _make_config(tmp_path),
        "tmp_path": tmp_path,
    }


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestProvision:
    async def test_happy_path(self, docker_env_kit):
        """provision() creates container, bootstraps, sets up workspace,
        injects secrets and credentials, records assignment, returns handle."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch(project_env={"KEY": "val"})
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=["claude"],
            ) as mock_inject_creds,
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            handle = await env.provision(dispatch, config)

        # Container created
        MockDockerConn.create_and_start.assert_awaited_once()
        create_kwargs = MockDockerConn.create_and_start.call_args
        assert create_kwargs.args[0] == docker_env_kit["docker_config"]

        # Connection check called
        mock_conn.check_connection.assert_awaited_once()

        # Bootstrap called
        docker_env_kit["bootstrapper"].bootstrap.assert_awaited_once_with(mock_conn)

        # Workspace setup called with empty setup_commands (clone only)
        docker_env_kit["workspace_mgr"].setup.assert_awaited_once()
        ws_spec = docker_env_kit["workspace_mgr"].setup.call_args.args[1]
        assert ws_spec.setup_commands == ()

        # Secrets injected
        docker_env_kit["workspace_mgr"].inject_secrets.assert_awaited_once()

        # CLI credentials injected
        mock_inject_creds.assert_awaited_once()

        # State assignment recorded
        docker_env_kit["state_store"].record_assignment.assert_awaited_once()

        # Handle returned with correct shape
        assert isinstance(handle, EnvironmentHandle)
        assert handle.project == "test-project"
        assert handle.branch == "main"
        assert handle.runtime.kind == "docker"
        assert handle.runtime.container_id == "abc123container"
        assert handle.runtime.connection is mock_conn

    async def test_cleanup_on_bootstrap_failure(self, docker_env_kit):
        """provision() cleans up container when bootstrap fails."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        docker_env_kit["bootstrapper"].bootstrap.side_effect = RuntimeError("bootstrap boom")

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            with pytest.raises(RuntimeError, match="bootstrap boom"):
                await env.provision(dispatch, config)

        # Container must be stopped and removed
        mock_conn.stop_container.assert_awaited_once()
        mock_conn.remove_container.assert_awaited_once()
        mock_conn.close.assert_awaited_once()

        # State store records release
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_cleanup_on_cancelled_error(self, docker_env_kit):
        """provision() cleans up container when cancelled (no orphaned containers)."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        docker_env_kit["bootstrapper"].bootstrap.side_effect = asyncio.CancelledError()

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            with pytest.raises(asyncio.CancelledError):
                await env.provision(dispatch, config)

        # Container must be cleaned up even on cancellation
        mock_conn.stop_container.assert_awaited_once()
        mock_conn.remove_container.assert_awaited_once()
        mock_conn.close.assert_awaited_once()
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_resource_limits_passed_to_connection(self, docker_env_kit):
        """provision() computes cpu_limit and memory_limit_bytes from profile.resources."""
        env = docker_env_kit["env"]
        profile = _make_docker_profile(
            resources=ResourceRequirements(cpu=4, memory_gb=8, gpu=False),
        )
        dispatch = _make_dispatch(resolved_profile=profile)
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        create_kwargs = MockDockerConn.create_and_start.call_args
        assert create_kwargs.kwargs["cpu_limit"] == pytest.approx(4.0)
        assert create_kwargs.kwargs["memory_limit_bytes"] == 8 * 1024**3

    async def test_missing_repo_url_raises(self, docker_env_kit):
        """provision() raises RuntimeError when no repo URL is configured."""
        env = docker_env_kit["env"]
        # Profile with no repo_url in docker_config and unknown project
        profile = _make_docker_profile(
            docker_config=DockerExecutionConfig(
                image="ubuntu:24.04",
                repo_url="",
            ),
        )
        dispatch = _make_dispatch(project="unknown-project", resolved_profile=profile)
        config = docker_env_kit["config"]

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            with pytest.raises(RuntimeError, match="No repo URL configured"):
                await env.provision(dispatch, config)

        # Container must still be cleaned up
        mock_conn.stop_container.assert_awaited_once()
        mock_conn.remove_container.assert_awaited_once()
        docker_env_kit["state_store"].record_release.assert_awaited_once()

    async def test_repo_url_from_profile_docker_config(self, docker_env_kit):
        """provision() prefers repo_url from profile.docker_config over instance mapping."""
        env = docker_env_kit["env"]
        profile = _make_docker_profile(
            docker_config=DockerExecutionConfig(
                image="ubuntu:24.04",
                repo_url="https://github.com/from-profile/repo.git",
            ),
        )
        dispatch = _make_dispatch(resolved_profile=profile)
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        # Workspace setup called with repo_url from profile
        ws_spec = docker_env_kit["workspace_mgr"].setup.call_args.args[1]
        assert ws_spec.repo_url == "https://github.com/from-profile/repo.git"

    async def test_connection_check_failure_raises(self, docker_env_kit):
        """provision() raises RuntimeError when container fails connectivity check."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = False
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            with pytest.raises(RuntimeError, match="failed connectivity check"):
                await env.provision(dispatch, config)

        mock_conn.stop_container.assert_awaited_once()
        mock_conn.remove_container.assert_awaited_once()

    async def test_provision_injects_mcp_config(self, docker_env_kit):
        """provision() calls inject_mcp_config when profile has MCP servers."""
        env = docker_env_kit["env"]
        config = docker_env_kit["config"]
        workspace_mgr = docker_env_kit["workspace_mgr"]

        mcp_profile = _make_docker_profile(
            mcp={
                "context7": McpServerConfig(
                    url="https://mcp.context7.com/sse",
                    headers={"Authorization": "MCP_CONTEXT7_KEY"},
                )
            },
        )
        dispatch = _make_dispatch(resolved_profile=mcp_profile)

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        workspace_mgr.inject_mcp_config.assert_awaited_once()
        call_args = workspace_mgr.inject_mcp_config.call_args
        assert "context7" in call_args.args[2]
        assert call_args.args[2]["context7"].url == "https://mcp.context7.com/sse"

    async def test_provision_skips_mcp_when_empty(self, docker_env_kit):
        """provision() does not call inject_mcp_config when profile has no MCP servers."""
        env = docker_env_kit["env"]
        config = docker_env_kit["config"]
        workspace_mgr = docker_env_kit["workspace_mgr"]

        dispatch = _make_dispatch(resolved_profile=_make_docker_profile(mcp={}))

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        workspace_mgr.inject_mcp_config.assert_not_awaited()

    async def test_provision_runs_setup_commands_as_agent_user(self, docker_env_kit):
        """provision() runs setup commands wrapped with su for agent user."""
        env = docker_env_kit["env"]
        profile = _make_docker_profile(setup=("make setup",))
        dispatch = _make_dispatch(resolved_profile=profile)
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        setup_calls = [c for c in mock_conn.run.call_args_list if "make setup" in str(c)]
        assert len(setup_calls) == 1
        assert "su - tanren -c" in setup_calls[0].args[0]

    async def test_provision_chowns_workspace_to_agent_user(self, docker_env_kit):
        """provision() transfers workspace ownership to agent user."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        chown_calls = [c for c in mock_conn.run.call_args_list if "chown" in str(c)]
        assert len(chown_calls) >= 1
        # First chown should be workspace ownership transfer
        assert "tanren:tanren" in chown_calls[0].args[0]

    async def test_provision_passes_target_home_to_credential_injection(self, docker_env_kit):
        """provision() passes agent_user home as target_home to credential injection."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=["claude"],
            ) as mock_inject,
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        mock_inject.assert_awaited_once()
        _, kwargs = mock_inject.call_args
        assert kwargs["target_home"] == "/home/tanren"

    async def test_provision_resolves_required_secrets(self, docker_env_kit, monkeypatch):
        """provision() resolves required_secrets from os.environ as developer_overrides."""
        monkeypatch.setenv("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-test-123")

        env = docker_env_kit["env"]
        dispatch = _make_dispatch(
            project_env={"PROJ_KEY": "val"},
            required_secrets=("CLAUDE_CODE_OAUTH_TOKEN",),
        )
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        call_kwargs = docker_env_kit["secret_loader"].build_bundle.call_args
        assert call_kwargs.kwargs["developer_overrides"] == {
            "CLAUDE_CODE_OAUTH_TOKEN": "sk-ant-test-123",
        }

    async def test_provision_no_required_secrets_passes_none(self, docker_env_kit):
        """Without required_secrets, developer_overrides is None."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            await env.provision(dispatch, config)

        call_kwargs = docker_env_kit["secret_loader"].build_bundle.call_args
        assert call_kwargs.kwargs.get("developer_overrides") is None

    async def test_provision_stores_workflow_id_in_runtime(self, docker_env_kit):
        """provision() stores workflow_id in the typed runtime context."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn,
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=[],
            ),
        ):
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            handle = await env.provision(dispatch, config)

        assert handle.runtime.kind == "docker"
        assert handle.runtime.workflow_id == "wf-test-project-42-1000"

    async def test_cleanup_records_release_even_when_stop_fails(self, docker_env_kit):
        """record_release is still called when container stop raises during provision cleanup."""
        env = docker_env_kit["env"]
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        docker_env_kit["bootstrapper"].bootstrap.side_effect = RuntimeError("boom")

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            mock_conn.stop_container.side_effect = RuntimeError("stop failed")
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            with pytest.raises(RuntimeError, match="boom"):
                await env.provision(dispatch, config)

        # stop was attempted
        mock_conn.stop_container.assert_awaited_once()
        # remove was still attempted
        mock_conn.remove_container.assert_awaited_once()
        # record_release still called despite stop failure
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_setup_command_failure_raises(self, docker_env_kit):
        """provision() raises RuntimeError when a setup command fails."""
        env = docker_env_kit["env"]
        profile = _make_docker_profile(setup=("failing-cmd",))
        dispatch = _make_dispatch(resolved_profile=profile)
        config = docker_env_kit["config"]

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            mock_conn = AsyncMock()
            mock_conn.container_id = "abc123container"
            mock_conn.check_connection.return_value = True
            # First call (chown) succeeds, second call (setup) fails
            mock_conn.run.side_effect = [
                _OK_REMOTE_RESULT,  # chown workspace
                RemoteResult(exit_code=1, stdout="", stderr="setup error", timed_out=False),
            ]
            MockDockerConn.create_and_start = AsyncMock(return_value=mock_conn)

            with pytest.raises(RuntimeError, match="Setup command failed"):
                await env.provision(dispatch, config)

        # Container cleaned up
        mock_conn.stop_container.assert_awaited_once()
        mock_conn.remove_container.assert_awaited_once()


class TestExecute:
    async def test_happy_path(self, docker_env_kit):
        """execute() runs agent and returns PhaseResult with correct outcome."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0, stdout="pushed", stderr="", timed_out=False
        )
        conn.download_content = AsyncMock(return_value="do the task")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.map_outcome") as mock_map,
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            mock_map.return_value = (Outcome.SUCCESS, "success")

            result = await env.execute(handle, dispatch, config)

        assert isinstance(result, PhaseResult)
        assert result.outcome == Outcome.SUCCESS
        assert result.signal == "success"
        docker_env_kit["runner"].run.assert_awaited_once()

    async def test_transient_retry(self, docker_env_kit):
        """execute() retries on transient errors."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="ok", stderr="", timed_out=False)
        conn.download_content = AsyncMock(return_value="prompt")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        # First two calls: transient error. Third call: success.
        transient_result = _make_agent_result(
            exit_code=1, stdout="rate limit 429", timed_out=False, signal_content=""
        )
        success_result = _make_agent_result(exit_code=0, signal_content="do-task-status: complete")
        docker_env_kit["runner"].run.side_effect = [
            transient_result,
            transient_result,
            success_result,
        ]

        with (
            patch(f"{_DOCKER_ENV}.map_outcome") as mock_map,
            patch(f"{_DOCKER_ENV}.classify_error") as mock_classify,
            patch("asyncio.sleep", new_callable=AsyncMock),
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            mock_map.side_effect = [
                (Outcome.ERROR, None),
                (Outcome.ERROR, None),
                (Outcome.SUCCESS, "complete"),
            ]
            mock_classify.return_value = ErrorClass.TRANSIENT

            result = await env.execute(handle, dispatch, config)

        assert result.outcome == Outcome.SUCCESS
        assert result.retries == 2
        assert docker_env_kit["runner"].run.await_count == 3

    async def test_push_on_push_phases(self, docker_env_kit):
        """execute() pushes to remote on DO_TASK phase."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        conn.download_content = AsyncMock(return_value="prompt")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        config = docker_env_kit["config"]

        docker_env_kit[
            "workspace_mgr"
        ].push_command.return_value = (
            "GIT_ASKPASS=/workspace/.git-askpass GIT_TERMINAL_PROMPT=0 git push origin main"
        )

        with (
            patch(f"{_DOCKER_ENV}.map_outcome") as mock_map,
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            mock_map.return_value = (Outcome.SUCCESS, "success")

            await env.execute(handle, dispatch, config)

        # push_command called with correct args
        docker_env_kit["workspace_mgr"].push_command.assert_called_once_with(
            "/workspace/test-project", "main"
        )
        # conn.run should have been called with the push command wrapped for agent user
        push_calls = [c for c in conn.run.call_args_list if "git push" in str(c)]
        assert len(push_calls) == 1

    async def test_skips_push_on_error_outcome(self, docker_env_kit):
        """execute() skips push when outcome is ERROR."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        conn.download_content = AsyncMock(return_value="prompt")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        config = docker_env_kit["config"]

        docker_env_kit["runner"].run.return_value = _make_agent_result(
            exit_code=1,
            stdout="fatal error",
            signal_content="do-task-status: error",
        )

        with (
            patch(f"{_DOCKER_ENV}.map_outcome") as mock_map,
            patch(f"{_DOCKER_ENV}.classify_error") as mock_classify,
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            mock_map.return_value = (Outcome.ERROR, "error")
            mock_classify.return_value = ErrorClass.FATAL

            await env.execute(handle, dispatch, config)

        # No push should have been called
        push_calls = [c for c in conn.run.call_args_list if "git push" in str(c)]
        assert len(push_calls) == 0

    async def test_execute_wraps_push_with_su(self, docker_env_kit):
        """execute() wraps git push with su when agent_user is set."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        conn.download_content = AsyncMock(return_value="prompt")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        config = docker_env_kit["config"]

        docker_env_kit["workspace_mgr"].push_command.return_value = "git push origin main"

        with (
            patch(f"{_DOCKER_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            await env.execute(handle, dispatch, config)

        push_calls = [c for c in conn.run.call_args_list if "git push" in str(c)]
        assert len(push_calls) == 1
        assert push_calls[0].args[0].startswith("su - tanren -c ")

    async def test_execute_collects_token_usage(self, docker_env_kit):
        """execute() collects token usage and populates result.token_usage."""
        from tanren_core.ccusage import TokenUsage

        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0, stdout="pushed", stderr="", timed_out=False
        )
        conn.download_content = AsyncMock(return_value="prompt")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.CLAUDE)
        config = docker_env_kit["config"]

        mock_usage = TokenUsage(
            input_tokens=100,
            output_tokens=200,
            total_tokens=300,
            total_cost=1.50,
            provider="claude",
        )

        with (
            patch(f"{_DOCKER_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(
                f"{_DOCKER_ENV}.collect_token_usage",
                new_callable=AsyncMock,
                return_value=mock_usage,
            ),
        ):
            result = await env.execute(handle, dispatch, config)

        assert result.token_usage is not None
        assert result.token_usage.total_cost == pytest.approx(1.50)
        assert result.token_usage.total_tokens == 300

    async def test_execute_skips_token_usage_for_bash(self, docker_env_kit):
        """execute() does not collect token usage for Cli.BASH dispatches."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="ok", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.BASH)
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock) as mock_collect,
        ):
            result = await env.execute(handle, dispatch, config)

        mock_collect.assert_not_awaited()
        assert result.token_usage is None

    async def test_execute_does_not_inject_cli_auth(self, docker_env_kit):
        """execute() does NOT inject CLI auth -- all auth happens at provision."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        conn.download_content = AsyncMock(return_value="prompt")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.CLAUDE)
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
            patch(
                f"{_DOCKER_ENV}.inject_all_cli_credentials", new_callable=AsyncMock
            ) as mock_inject,
        ):
            await env.execute(handle, dispatch, config)

        mock_inject.assert_not_awaited()

    async def test_execute_wrong_runtime_kind_raises(self, docker_env_kit):
        """execute() raises RuntimeError if handle has wrong runtime kind."""
        from tanren_core.adapters.types import LocalEnvironmentRuntime

        env = docker_env_kit["env"]
        handle = EnvironmentHandle(
            env_id="env-test-1",
            worktree_path=Path("/workspace/test"),
            branch="main",
            project="test-project",
            runtime=LocalEnvironmentRuntime(),
        )
        dispatch = _make_dispatch()
        config = docker_env_kit["config"]

        with pytest.raises(RuntimeError, match="requires docker runtime handle"):
            await env.execute(handle, dispatch, config)

    async def test_execute_passes_agent_user_to_ccusage_runner(self, docker_env_kit):
        """execute() passes agent_user as run_as_user to RemoteCommandRunner."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        conn.download_content = AsyncMock(return_value="prompt")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.CLAUDE)
        config = docker_env_kit["config"]

        with (
            patch(f"{_DOCKER_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_DOCKER_ENV}.RemoteCommandRunner") as MockRunner,
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            await env.execute(handle, dispatch, config)

        MockRunner.assert_called_once_with(conn, run_as_user="tanren")

    async def test_push_failure_includes_diagnostic_in_stdout(self, docker_env_kit):
        """execute() appends push failure diagnostic to stdout."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        # Push fails
        conn.run.return_value = RemoteResult(
            exit_code=1, stdout="", stderr="push rejected", timed_out=False
        )
        conn.download_content = AsyncMock(return_value="prompt")
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        config = docker_env_kit["config"]

        docker_env_kit["runner"].run.return_value = _make_agent_result(
            stdout="agent output", signal_content="do-task-status: complete"
        )

        with (
            patch(f"{_DOCKER_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_DOCKER_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            result = await env.execute(handle, dispatch, config)

        assert "Remote git push failed" in result.stdout
        assert result.postflight_result is not None
        assert result.postflight_result.pushed is False

    async def test_gate_phase_captures_output(self, docker_env_kit):
        """execute() captures gate output for gate phases."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.GATE, cli=Cli.BASH)
        config = docker_env_kit["config"]

        docker_env_kit["runner"].run.return_value = _make_agent_result(
            stdout="gate output here", stderr="gate stderr"
        )

        with patch(f"{_DOCKER_ENV}.map_outcome", return_value=(Outcome.SUCCESS, None)):
            result = await env.execute(handle, dispatch, config)

        assert result.gate_output is not None
        assert "gate output here" in result.gate_output


class TestTeardown:
    async def test_happy_path(self, docker_env_kit):
        """teardown() runs teardown commands, cleans workspace, stops/removes
        container, records release."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = _OK_REMOTE_RESULT
        workspace = _make_workspace()
        handle = _make_handle(conn=conn, workspace=workspace)

        await env.teardown(handle)

        # Teardown commands executed
        teardown_calls = [c for c in conn.run.call_args_list if "make clean" in str(c)]
        assert len(teardown_calls) == 1

        # Workspace cleaned
        docker_env_kit["workspace_mgr"].cleanup.assert_awaited_once_with(conn, workspace)

        # Container stopped and removed
        conn.stop_container.assert_awaited_once()
        conn.remove_container.assert_awaited_once()

        # Connection closed
        conn.close.assert_awaited_once()

        # State store records release
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_cleanup_continues_on_teardown_command_failure(self, docker_env_kit):
        """teardown() continues cleanup when teardown commands fail."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.side_effect = RuntimeError("teardown cmd failed")
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        # Container still stopped/removed despite teardown command failure
        conn.stop_container.assert_awaited_once()
        conn.remove_container.assert_awaited_once()
        conn.close.assert_awaited_once()
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_release_recorded_even_on_container_stop_error(self, docker_env_kit):
        """record_release is still called when container stop fails."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = _OK_REMOTE_RESULT
        conn.stop_container.side_effect = RuntimeError("stop boom")
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        conn.stop_container.assert_awaited_once()
        # remove still attempted
        conn.remove_container.assert_awaited_once()
        conn.close.assert_awaited_once()
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_release_recorded_even_on_container_remove_error(self, docker_env_kit):
        """record_release is still called when container remove fails."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = _OK_REMOTE_RESULT
        conn.remove_container.side_effect = RuntimeError("remove boom")
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        conn.stop_container.assert_awaited_once()
        conn.remove_container.assert_awaited_once()
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_release_recorded_even_on_close_error(self, docker_env_kit):
        """record_release is still called when connection close fails."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = _OK_REMOTE_RESULT
        conn.close.side_effect = RuntimeError("close boom")
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        conn.close.assert_awaited_once()
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_workspace_cleanup_failure_still_stops_container(self, docker_env_kit):
        """teardown() stops container even when workspace cleanup fails."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = _OK_REMOTE_RESULT
        docker_env_kit["workspace_mgr"].cleanup.side_effect = RuntimeError("cleanup boom")
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        conn.stop_container.assert_awaited_once()
        conn.remove_container.assert_awaited_once()
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("abc123container")

    async def test_teardown_wraps_commands_with_su(self, docker_env_kit):
        """teardown() wraps user teardown commands with su when agent_user is set."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = _OK_REMOTE_RESULT
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        teardown_calls = [c for c in conn.run.call_args_list if "make clean" in str(c)]
        assert len(teardown_calls) == 1
        assert teardown_calls[0].args[0].startswith("su - tanren -c ")

    async def test_teardown_wrong_runtime_kind_raises(self, docker_env_kit):
        """teardown() raises RuntimeError if handle has wrong runtime kind."""
        from tanren_core.adapters.types import LocalEnvironmentRuntime

        env = docker_env_kit["env"]
        handle = EnvironmentHandle(
            env_id="env-test-1",
            worktree_path=Path("/workspace/test"),
            branch="main",
            project="test-project",
            runtime=LocalEnvironmentRuntime(),
        )

        with pytest.raises(RuntimeError, match="requires docker runtime handle"):
            await env.teardown(handle)

    async def test_teardown_cleans_credential_files(self, docker_env_kit):
        """teardown() removes credential files using absolute paths."""
        env = docker_env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = _OK_REMOTE_RESULT
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        # Check that credential cleanup uses absolute paths (not tilde)
        rm_calls = [
            call.args[0] for call in conn.run.call_args_list if call.args[0].startswith("rm -f ")
        ]
        for rm_cmd in rm_calls:
            path = rm_cmd.replace("rm -f ", "")
            assert path.startswith("/home/tanren"), f"Expected absolute path, got: {path}"
            assert "~" not in path


class TestGetAccessInfo:
    async def test_returns_working_dir(self, docker_env_kit):
        """get_access_info() returns AccessInfo with working_dir and status."""
        env = docker_env_kit["env"]
        handle = _make_handle()

        info = await env.get_access_info(handle)

        assert isinstance(info, AccessInfo)
        assert info.working_dir == "/workspace/test-project"
        assert info.status == "running"

    async def test_wrong_runtime_kind_raises(self, docker_env_kit):
        """get_access_info() raises RuntimeError if handle has wrong runtime kind."""
        from tanren_core.adapters.types import LocalEnvironmentRuntime

        env = docker_env_kit["env"]
        handle = EnvironmentHandle(
            env_id="env-test-1",
            worktree_path=Path("/workspace/test"),
            branch="main",
            project="test-project",
            runtime=LocalEnvironmentRuntime(),
        )

        with pytest.raises(RuntimeError, match="requires docker runtime handle"):
            await env.get_access_info(handle)


class TestRecoverStaleAssignments:
    async def test_no_assignments_returns_zero(self, docker_env_kit):
        """Returns 0 and does not call release when no stale assignments exist."""
        env = docker_env_kit["env"]
        docker_env_kit["state_store"].get_active_assignments.return_value = []

        result = await env.recover_stale_assignments()

        assert result == 0
        docker_env_kit["state_store"].record_release.assert_not_awaited()

    async def test_recovers_assignments(self, docker_env_kit):
        """Recovers stale assignments: stops/removes containers and records release."""
        env = docker_env_kit["env"]
        assignments = [
            VMAssignment(
                vm_id="container-1",
                workflow_id="wf-1",
                project="proj",
                spec="spec",
                host="local",
                assigned_at="2026-01-01T00:00:00Z",
            ),
            VMAssignment(
                vm_id="container-2",
                workflow_id="wf-2",
                project="proj",
                spec="spec",
                host="local",
                assigned_at="2026-01-01T00:00:00Z",
            ),
        ]
        docker_env_kit["state_store"].get_active_assignments.return_value = assignments

        mock_conn = AsyncMock()

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            MockDockerConn.from_existing.return_value = mock_conn

            result = await env.recover_stale_assignments()

        assert result == 2
        assert MockDockerConn.from_existing.call_count == 2
        assert mock_conn.stop_container.await_count == 2
        assert mock_conn.remove_container.await_count == 2
        assert mock_conn.close.await_count == 2
        assert docker_env_kit["state_store"].record_release.await_count == 2
        docker_env_kit["state_store"].record_release.assert_any_await("container-1")
        docker_env_kit["state_store"].record_release.assert_any_await("container-2")

    async def test_records_release_on_stop_failure(self, docker_env_kit):
        """record_release is still called when container stop raises during recovery."""
        env = docker_env_kit["env"]
        assignments = [
            VMAssignment(
                vm_id="container-1",
                workflow_id="wf-1",
                project="proj",
                spec="spec",
                host="local",
                assigned_at="2026-01-01T00:00:00Z",
            ),
        ]
        docker_env_kit["state_store"].get_active_assignments.return_value = assignments

        mock_conn = AsyncMock()
        mock_conn.stop_container.side_effect = RuntimeError("stop failed")

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            MockDockerConn.from_existing.return_value = mock_conn

            result = await env.recover_stale_assignments()

        assert result == 1
        # stop was attempted
        mock_conn.stop_container.assert_awaited_once()
        # remove still called
        mock_conn.remove_container.assert_awaited_once()
        # release recorded despite failure
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("container-1")

    async def test_records_release_on_remove_failure(self, docker_env_kit):
        """record_release is still called when container remove raises during recovery."""
        env = docker_env_kit["env"]
        assignments = [
            VMAssignment(
                vm_id="container-1",
                workflow_id="wf-1",
                project="proj",
                spec="spec",
                host="local",
                assigned_at="2026-01-01T00:00:00Z",
            ),
        ]
        docker_env_kit["state_store"].get_active_assignments.return_value = assignments

        mock_conn = AsyncMock()
        mock_conn.remove_container.side_effect = RuntimeError("remove failed")

        with patch(f"{_DOCKER_ENV}.DockerConnection") as MockDockerConn:
            MockDockerConn.from_existing.return_value = mock_conn

            result = await env.recover_stale_assignments()

        assert result == 1
        mock_conn.close.assert_awaited_once()
        docker_env_kit["state_store"].record_release.assert_awaited_once_with("container-1")


class TestDryRun:
    async def test_returns_manual_provider(self, docker_env_kit):
        """dry_run() returns DryRunInfo with MANUAL provider."""
        env = docker_env_kit["env"]
        requirements = VMRequirements(profile="default", cpu=2, memory_gb=4, gpu=False)

        result = await env.dry_run(requirements)

        assert isinstance(result, DryRunInfo)
        assert result.provider == VMProvider.MANUAL
        assert result.would_provision is True


class TestClose:
    async def test_close_closes_state_store(self, docker_env_kit):
        """close() delegates to _state_store.close()."""
        env = docker_env_kit["env"]
        await env.close()
        docker_env_kit["state_store"].close.assert_awaited_once()
