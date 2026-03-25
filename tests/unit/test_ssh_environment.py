"""Tests for SSHExecutionEnvironment composition layer."""

from __future__ import annotations

import time
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from tanren_core.adapters.remote_shared import extract_signal_token, validate_cli_auth
from tanren_core.adapters.remote_types import (
    BootstrapResult,
    RemoteAgentResult,
    RemoteResult,
    SecretBundle,
    VMHandle,
    VMProvider,
    WorkspacePath,
)
from tanren_core.adapters.ssh import SSHConfig
from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment
from tanren_core.adapters.types import (
    AccessInfo,
    EnvironmentHandle,
    PhaseResult,
    RemoteEnvironmentRuntime,
)
from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    EnvironmentProfileType,
    ResourceRequirements,
)
from tanren_core.errors import ErrorClass
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase
from tanren_core.worker_config import WorkerConfig

_SSH_ENV = "tanren_core.adapters.ssh_environment"

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
    project: str = "myproj",
    branch: str = "feature-1",
    context: str = "Do the work",
    resolved_profile: EnvironmentProfile | None = None,
    project_env: dict[str, str] | None = None,
    cloud_secrets: dict[str, str] | None = None,
) -> Dispatch:
    return Dispatch(
        workflow_id="wf-myproj-42-1000",
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
        resolved_profile=resolved_profile or DEFAULT_PROFILE,
        project_env=project_env or {},
        cloud_secrets=cloud_secrets or {},
    )


def _make_vm_handle() -> VMHandle:
    return VMHandle(
        vm_id="vm-abc-123",
        host="10.0.0.42",
        provider=VMProvider.HETZNER,
        created_at="2025-01-01T00:00:00Z",
        hourly_cost=0.50,
    )


def _make_workspace() -> WorkspacePath:
    return WorkspacePath(path="/workspace/myproj", project="myproj", branch="feature-1")


def _make_bootstrap_result() -> BootstrapResult:
    return BootstrapResult(
        installed=("uv", "claude"),
        skipped=("git",),
        duration_secs=12,
    )


def _make_profile() -> EnvironmentProfile:
    return EnvironmentProfile(
        name="default",
        type=EnvironmentProfileType.REMOTE,
        resources=ResourceRequirements(cpu=2, memory_gb=4, gpu=False),
        setup=("make setup",),
        teardown=("make clean",),
    )


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


# ---------------------------------------------------------------------------
# Fixture
# ---------------------------------------------------------------------------


@pytest.fixture
def env_kit(tmp_path: Path):
    """Build an SSHExecutionEnvironment with all sub-adapters mocked."""
    vm_provisioner = AsyncMock()
    bootstrapper = AsyncMock()
    workspace_mgr = AsyncMock()
    runner = AsyncMock()
    state_store = AsyncMock()
    secret_loader = MagicMock()

    vm_provisioner.acquire.return_value = _make_vm_handle()
    vm_provisioner.release.return_value = None
    bootstrapper.bootstrap.return_value = _make_bootstrap_result()
    workspace_mgr.setup.return_value = _make_workspace()
    workspace_mgr.inject_secrets.return_value = None
    workspace_mgr.cleanup.return_value = None
    workspace_mgr.push_command = MagicMock(return_value="git push origin main")
    runner.run.return_value = _make_agent_result()
    secret_loader.build_bundle.return_value = SecretBundle()
    state_store.record_assignment.return_value = None
    state_store.record_release.return_value = None

    ssh_defaults = SSHConfig(
        host="placeholder",
        user="dev",
        key_path="~/.ssh/id_rsa",
        port=22,
        connect_timeout=10,
    )

    env = SSHExecutionEnvironment(
        vm_provisioner=vm_provisioner,
        bootstrapper=bootstrapper,
        workspace_mgr=workspace_mgr,
        runner=runner,
        state_store=state_store,
        secret_loader=secret_loader,
        ssh_config_defaults=ssh_defaults,
        repo_urls={"myproj": "git@github.com:org/myproj.git"},
        provider=VMProvider.HETZNER,
    )

    return {
        "env": env,
        "vm_provisioner": vm_provisioner,
        "bootstrapper": bootstrapper,
        "workspace_mgr": workspace_mgr,
        "runner": runner,
        "state_store": state_store,
        "secret_loader": secret_loader,
        "config": _make_config(tmp_path),
        "tmp_path": tmp_path,
    }


def _make_handle(conn=None, vm_handle=None, workspace=None) -> EnvironmentHandle:
    """Build a minimal EnvironmentHandle for execute/teardown tests."""
    return EnvironmentHandle(
        env_id="env-test-1",
        worktree_path=Path("/workspace/myproj"),
        branch="feature-1",
        project="myproj",
        runtime=RemoteEnvironmentRuntime(
            vm_handle=vm_handle or _make_vm_handle(),
            connection=conn or AsyncMock(),
            workspace_path=workspace or _make_workspace(),
            profile=_make_profile(),
            teardown_commands=("make clean",),
            provision_start=time.monotonic(),
            workflow_id="wf-myproj-42-1000",
        ),
    )


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestRecoverStaleAssignments:
    async def test_recover_stale_no_assignments(self, env_kit):
        """Returns 0 and does not call release when no stale assignments exist."""
        env = env_kit["env"]
        env_kit["state_store"].get_active_assignments.return_value = []

        result = await env.recover_stale_assignments()

        assert result == 0
        env_kit["vm_provisioner"].release.assert_not_awaited()
        env_kit["state_store"].record_release.assert_not_awaited()

    async def test_recover_stale_releases_and_records(self, env_kit):
        """Releases each stale assignment and records release for all."""
        from tanren_core.adapters.remote_types import (
            VMAssignment,
        )

        env = env_kit["env"]
        assignments = [
            VMAssignment(
                vm_id="vm-1",
                workflow_id="wf-1",
                project="proj",
                spec="spec",
                host="10.0.0.1",
                assigned_at="2026-01-01T00:00:00Z",
            ),
            VMAssignment(
                vm_id="vm-2",
                workflow_id="wf-2",
                project="proj",
                spec="spec",
                host="10.0.0.2",
                assigned_at="2026-01-01T00:00:00Z",
            ),
        ]
        env_kit["state_store"].get_active_assignments.return_value = assignments

        result = await env.recover_stale_assignments()

        assert result == 2
        assert env_kit["vm_provisioner"].release.await_count == 2
        assert env_kit["state_store"].record_release.await_count == 2
        env_kit["state_store"].record_release.assert_any_await("vm-1")
        env_kit["state_store"].record_release.assert_any_await("vm-2")

    async def test_recover_stale_records_release_on_provider_failure(self, env_kit):
        """record_release is still called when provider release raises."""
        from tanren_core.adapters.remote_types import (
            VMAssignment,
        )

        env = env_kit["env"]
        assignments = [
            VMAssignment(
                vm_id="vm-1",
                workflow_id="wf-1",
                project="proj",
                spec="spec",
                host="10.0.0.1",
                assigned_at="2026-01-01T00:00:00Z",
            ),
        ]
        env_kit["state_store"].get_active_assignments.return_value = assignments
        env_kit["vm_provisioner"].release.side_effect = RuntimeError("provider down")

        result = await env.recover_stale_assignments()

        assert result == 1
        env_kit["state_store"].record_release.assert_awaited_once_with("vm-1")

    async def test_recover_stale_uses_configured_provider(self, env_kit):
        """recover_stale_assignments uses the configured provider, not hardcoded MANUAL."""
        from tanren_core.adapters.remote_types import (
            VMAssignment,
        )

        env = env_kit["env"]
        assignments = [
            VMAssignment(
                vm_id="vm-1",
                workflow_id="wf-1",
                project="proj",
                spec="spec",
                host="10.0.0.1",
                assigned_at="2026-01-01T00:00:00Z",
            ),
        ]
        env_kit["state_store"].get_active_assignments.return_value = assignments

        await env.recover_stale_assignments()

        released_handle = env_kit["vm_provisioner"].release.call_args[0][0]
        assert released_handle.provider == VMProvider.HETZNER


class TestProvision:
    async def test_acquires_vm_bootstraps_workspace_returns_handle(self, env_kit):
        """provision() acquires VM, creates SSH conn, bootstraps, sets up
        workspace, injects secrets, records assignment, and returns handle."""
        env = env_kit["env"]
        dispatch = _make_dispatch(
            resolved_profile=_make_profile(),
            project_env={"KEY": "val"},
        )
        config = env_kit["config"]

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn

            handle = await env.provision(dispatch, config)

        # VM acquired
        env_kit["vm_provisioner"].acquire.assert_awaited_once()

        # SSH connection created with correct host
        MockSSH.assert_called_once()
        ssh_config_arg = MockSSH.call_args.args[0]
        assert ssh_config_arg.host == "10.0.0.42"

        # Bootstrap called
        env_kit["bootstrapper"].bootstrap.assert_awaited_once_with(mock_conn)

        # Workspace setup called with empty setup_commands (clone only)
        env_kit["workspace_mgr"].setup.assert_awaited_once()
        ws_spec = env_kit["workspace_mgr"].setup.call_args.args[1]
        assert ws_spec.setup_commands == ()

        # Setup commands run via conn.run as su - agent user
        setup_calls = [c for c in mock_conn.run.call_args_list if "make setup" in str(c)]
        assert len(setup_calls) == 1

        # Secrets injected
        env_kit["workspace_mgr"].inject_secrets.assert_awaited_once()

        # State assignment recorded
        env_kit["state_store"].record_assignment.assert_awaited_once()

        # Handle returned with correct shape
        assert isinstance(handle, EnvironmentHandle)
        assert handle.project == "myproj"
        assert handle.branch == "feature-1"
        assert handle.runtime.kind == "remote"
        assert handle.runtime.vm_handle == _make_vm_handle()
        assert handle.runtime.connection is mock_conn

    async def test_releases_vm_on_cancelled_error(self, env_kit):
        """provision() releases VM when cancelled during provisioning (no orphaned VMs)."""
        env = env_kit["env"]
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]

        import asyncio

        # Make bootstrap raise CancelledError (simulates task cancellation)
        env_kit["bootstrapper"].bootstrap.side_effect = asyncio.CancelledError()

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            MockSSH.return_value = mock_conn

            with pytest.raises(asyncio.CancelledError):
                await env.provision(dispatch, config)

        # VM must be released even though cancellation occurred
        env_kit["vm_provisioner"].release.assert_awaited_once_with(_make_vm_handle())
        # SSH connection closed during cleanup
        mock_conn.close.assert_awaited_once()

    async def test_releases_vm_on_failure(self, env_kit):
        """provision() releases VM when a step fails (no orphaned VMs)."""
        env = env_kit["env"]
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]

        # Make bootstrap fail
        env_kit["bootstrapper"].bootstrap.side_effect = RuntimeError("bootstrap boom")

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            MockSSH.return_value = mock_conn

            with pytest.raises(RuntimeError, match="bootstrap boom"):
                await env.provision(dispatch, config)

        # VM must be released even though bootstrap failed
        env_kit["vm_provisioner"].release.assert_awaited_once_with(_make_vm_handle())
        # SSH connection closed during cleanup
        mock_conn.close.assert_awaited_once()

    async def test_provision_cleanup_records_release_when_provider_release_fails(self, env_kit):
        """record_release is still called when provider release raises during provision cleanup."""
        env = env_kit["env"]
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]

        # Make bootstrap fail (triggering cleanup)
        env_kit["bootstrapper"].bootstrap.side_effect = RuntimeError("bootstrap boom")
        # Make release also fail
        env_kit["vm_provisioner"].release.side_effect = RuntimeError("provider down")

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            MockSSH.return_value = mock_conn

            with pytest.raises(RuntimeError, match="bootstrap boom"):
                await env.provision(dispatch, config)

        # release was attempted
        env_kit["vm_provisioner"].release.assert_awaited_once()
        # record_release still called despite release failure
        env_kit["state_store"].record_release.assert_awaited_once_with("vm-abc-123")

    async def test_raises_when_no_repo_url(self, env_kit):
        """provision() raises RuntimeError when no repo URL is configured."""
        env = env_kit["env"]
        dispatch = _make_dispatch(
            project="unknown-project",
            resolved_profile=_make_profile(),
        )
        config = env_kit["config"]

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            MockSSH.return_value = AsyncMock()

            with pytest.raises(RuntimeError, match="No repo URL configured"):
                await env.provision(dispatch, config)

        # VM must still be released
        env_kit["vm_provisioner"].release.assert_awaited_once()

    async def test_waits_for_ssh_before_bootstrap(self, env_kit):
        """provision() calls _await_ssh_ready between SSH creation and bootstrap."""
        env = env_kit["env"]
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]
        call_order: list[str] = []

        async def _track_ssh_ready(conn, **kwargs):  # noqa: RUF029 — async required by interface
            call_order.append("ssh_ready")

        async def _track_bootstrap(conn):  # noqa: RUF029 — async required by interface
            call_order.append("bootstrap")
            return _make_bootstrap_result()

        with (
            patch.object(env, "_await_ssh_ready", side_effect=_track_ssh_ready),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn
            env_kit["bootstrapper"].bootstrap.side_effect = _track_bootstrap
            await env.provision(dispatch, config)

        assert call_order == ["ssh_ready", "bootstrap"]

    async def test_releases_vm_on_ssh_timeout(self, env_kit):
        """provision() releases VM when SSH never becomes reachable."""
        env = env_kit["env"]
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]

        with (
            patch.object(
                env,
                "_await_ssh_ready",
                new_callable=AsyncMock,
                side_effect=TimeoutError("SSH not reachable"),
            ),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            MockSSH.return_value = mock_conn

            with pytest.raises(TimeoutError, match="SSH not reachable"):
                await env.provision(dispatch, config)

        # VM must be released on SSH timeout
        env_kit["vm_provisioner"].release.assert_awaited_once_with(_make_vm_handle())
        mock_conn.close.assert_awaited_once()

    async def test_provision_uses_dispatch_resolved_profile(self, env_kit):
        """provision() uses resolved_profile from dispatch, not file-based resolution."""
        env = env_kit["env"]
        profile = _make_profile()
        dispatch = _make_dispatch(resolved_profile=profile)
        config = env_kit["config"]

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn
            handle = await env.provision(dispatch, config)

        # The handle's runtime profile should match what was on the dispatch
        assert handle.runtime.profile == profile
        assert handle.runtime.teardown_commands == profile.teardown

    async def test_provision_resolves_required_secrets_from_environ(self, env_kit, monkeypatch):
        """provision() resolves required_secrets from os.environ as developer_overrides."""
        monkeypatch.setenv("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-test-123")
        monkeypatch.setenv("MCP_CONTEXT7_KEY", "ctx7-test-456")

        env = env_kit["env"]
        dispatch = _make_dispatch(
            resolved_profile=_make_profile(),
            project_env={"PROJ_KEY": "val"},
        )
        # Inject required_secrets into dispatch
        dispatch = dispatch.model_copy(
            update={"required_secrets": ("CLAUDE_CODE_OAUTH_TOKEN", "MCP_CONTEXT7_KEY")}
        )
        config = env_kit["config"]

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn
            await env.provision(dispatch, config)

        # build_bundle called with developer_overrides from os.environ
        call_kwargs = env_kit["secret_loader"].build_bundle.call_args
        assert call_kwargs.kwargs["developer_overrides"] == {
            "CLAUDE_CODE_OAUTH_TOKEN": "sk-ant-test-123",
            "MCP_CONTEXT7_KEY": "ctx7-test-456",
        }

    async def test_provision_no_required_secrets_uses_filesystem(self, env_kit):
        """Without required_secrets, falls back to filesystem loading."""
        env = env_kit["env"]
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn
            await env.provision(dispatch, config)

        call_kwargs = env_kit["secret_loader"].build_bundle.call_args
        assert call_kwargs.kwargs.get("developer_overrides") is None


class TestExecute:
    async def test_runs_agent_returns_phase_result(self, env_kit):
        """execute() runs agent and returns PhaseResult."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="pushed",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = env_kit["config"]

        with (
            patch(f"{_SSH_ENV}.map_outcome") as mock_map,
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            mock_map.return_value = (Outcome.SUCCESS, "success")

            result = await env.execute(handle, dispatch, config)

        assert isinstance(result, PhaseResult)
        assert result.outcome == Outcome.SUCCESS
        assert result.signal == "success"
        env_kit["runner"].run.assert_awaited_once()

    async def test_retries_on_transient_error(self, env_kit):
        """execute() retries on transient errors up to 3 times."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="ok",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = env_kit["config"]

        # First two calls: transient error. Third call: success.
        transient_result = _make_agent_result(
            exit_code=1,
            stdout="rate limit 429",
            timed_out=False,
            signal_content="",
        )
        success_result = _make_agent_result(exit_code=0, signal_content="do-task-status: complete")
        env_kit["runner"].run.side_effect = [
            transient_result,
            transient_result,
            success_result,
        ]

        with (
            patch(f"{_SSH_ENV}.map_outcome") as mock_map,
            patch(f"{_SSH_ENV}.classify_error") as mock_classify,
            patch("asyncio.sleep", new_callable=AsyncMock),
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
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
        assert env_kit["runner"].run.await_count == 3

    async def test_pushes_on_push_phases(self, env_kit):
        """execute() pushes to remote on push phases using workspace_mgr.push_command()."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        config = env_kit["config"]

        env_kit[
            "workspace_mgr"
        ].push_command.return_value = (
            "GIT_ASKPASS=/workspace/.git-askpass GIT_TERMINAL_PROMPT=0 git push origin feature-1"
        )

        with (
            patch(f"{_SSH_ENV}.map_outcome") as mock_map,
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            mock_map.return_value = (Outcome.SUCCESS, "success")

            await env.execute(handle, dispatch, config)

        # push_command called with correct args
        env_kit["workspace_mgr"].push_command.assert_called_once_with(
            "/workspace/myproj", "feature-1"
        )
        # conn.run should have been called with the auth-prefixed push command
        push_calls = [c for c in conn.run.call_args_list if "git push" in str(c)]
        assert len(push_calls) == 1
        assert "GIT_ASKPASS" in str(push_calls[0])

    async def test_skips_push_on_error_outcome(self, env_kit):
        """execute() skips push when outcome is ERROR or TIMEOUT."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        config = env_kit["config"]

        env_kit["runner"].run.return_value = _make_agent_result(
            exit_code=1,
            stdout="fatal error",
            signal_content="do-task-status: error",
        )

        with (
            patch(f"{_SSH_ENV}.map_outcome") as mock_map,
            patch(f"{_SSH_ENV}.classify_error") as mock_classify,
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            mock_map.return_value = (Outcome.ERROR, "error")
            mock_classify.return_value = ErrorClass.FATAL

            await env.execute(handle, dispatch, config)

        # conn.run should NOT have been called for git push
        push_calls = [c for c in conn.run.call_args_list if "git push" in str(c)]
        assert len(push_calls) == 0

    async def test_execute_collects_token_usage(self, env_kit):
        """execute() collects token usage and populates result.token_usage."""
        from tanren_core.ccusage import (
            TokenUsage,
        )

        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0, stdout="pushed", stderr="", timed_out=False
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.CLAUDE)
        config = env_kit["config"]

        mock_usage = TokenUsage(
            input_tokens=100,
            output_tokens=200,
            total_tokens=300,
            total_cost=1.50,
            provider="claude",
        )

        with (
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(
                f"{_SSH_ENV}.collect_token_usage",
                new_callable=AsyncMock,
                return_value=mock_usage,
            ),
        ):
            result = await env.execute(handle, dispatch, config)

        assert result.token_usage is not None
        assert result.token_usage.total_cost == pytest.approx(1.50)
        assert result.token_usage.total_tokens == 300

    async def test_execute_skips_token_usage_for_bash(self, env_kit):
        """execute() does not collect token usage for Cli.BASH dispatches."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="ok", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.BASH)
        config = env_kit["config"]

        with (
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock) as mock_collect,
        ):
            result = await env.execute(handle, dispatch, config)

        mock_collect.assert_not_awaited()
        assert result.token_usage is None

    async def test_execute_token_usage_failure_graceful(self, env_kit):
        """execute() returns None token_usage when collection returns None."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0, stdout="pushed", stderr="", timed_out=False
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.CLAUDE)
        config = env_kit["config"]

        with (
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(
                f"{_SSH_ENV}.collect_token_usage",
                new_callable=AsyncMock,
                return_value=None,
            ),
        ):
            result = await env.execute(handle, dispatch, config)

        assert result.token_usage is None
        assert result.outcome == Outcome.SUCCESS

    async def test_execute_does_not_inject_cli_auth(self, env_kit):
        """execute() does NOT inject CLI auth — all auth happens at provision."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.OPENCODE)
        config = env_kit["config"]

        with (
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
            patch(f"{_SSH_ENV}.inject_all_cli_credentials", new_callable=AsyncMock) as mock_inject,
        ):
            await env.execute(handle, dispatch, config)

        mock_inject.assert_not_awaited()


class TestExtractSignalToken:
    def test_extracts_from_file_content(self):
        token = extract_signal_token("do-task", "do-task-status: complete", "")
        assert token == "complete"

    def test_falls_back_to_stdout(self):
        token = extract_signal_token("do-task", "", "output\ndo-task-status: all-done\n")
        assert token == "all-done"

    def test_file_takes_precedence(self):
        token = extract_signal_token(
            "do-task", "do-task-status: complete", "do-task-status: blocked"
        )
        assert token == "complete"

    def test_returns_none_when_no_signal(self):
        token = extract_signal_token("do-task", "", "no signal here")
        assert token is None

    def test_last_stdout_match_wins(self):
        token = extract_signal_token(
            "do-task", "", "do-task-status: error\ndo-task-status: complete"
        )
        assert token == "complete"

    def test_works_with_audit_task(self):
        token = extract_signal_token("audit-task", "audit-task-status: pass", "")
        assert token == "pass"

    def test_whitespace_only_file_falls_through(self):
        token = extract_signal_token("do-task", "  \n  ", "do-task-status: blocked")
        assert token == "blocked"

    def test_file_has_wrong_command_name(self):
        token = extract_signal_token(
            "do-task", "audit-task-status: pass", "do-task-status: complete"
        )
        assert token == "complete"

    def test_empty_everything(self):
        assert extract_signal_token("do-task", "", "") is None


class TestValidateCliAuth:
    def test_claude_oauth_token_sufficient(self):
        validate_cli_auth(Cli.CLAUDE, {"CLAUDE_CODE_OAUTH_TOKEN": "tok"})

    def test_claude_credentials_json_sufficient(self):
        validate_cli_auth(Cli.CLAUDE, {"CLAUDE_CREDENTIALS_JSON": "{}"})

    def test_claude_both_present(self):
        validate_cli_auth(
            Cli.CLAUDE,
            {"CLAUDE_CODE_OAUTH_TOKEN": "tok", "CLAUDE_CREDENTIALS_JSON": "{}"},
        )

    def test_claude_neither_raises(self):
        with pytest.raises(RuntimeError, match="No auth secret resolved for claude"):
            validate_cli_auth(Cli.CLAUDE, {})

    def test_opencode_present(self):
        validate_cli_auth(Cli.OPENCODE, {"OPENCODE_ZAI_API_KEY": "key"})

    def test_opencode_missing_raises(self):
        with pytest.raises(RuntimeError, match="No auth secret resolved for opencode"):
            validate_cli_auth(Cli.OPENCODE, {})

    def test_codex_missing_raises(self):
        with pytest.raises(RuntimeError, match="No auth secret resolved for codex"):
            validate_cli_auth(Cli.CODEX, {})

    def test_bash_no_auth_needed(self):
        validate_cli_auth(Cli.BASH, {})

    def test_unrelated_secrets_dont_satisfy(self):
        with pytest.raises(RuntimeError, match="No auth secret resolved for claude"):
            validate_cli_auth(Cli.CLAUDE, {"SOME_OTHER_KEY": "val"})


class TestSignalExtraction:
    async def test_stdout_fallback_when_file_empty(self, env_kit):
        """Signal extracted from stdout when .agent-status file is empty."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="pushed",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = env_kit["config"]

        env_kit["runner"].run.return_value = _make_agent_result(
            signal_content="",
            stdout="some output\ndo-task-status: all-done\n",
        )

        with patch(
            f"{_SSH_ENV}.collect_token_usage",
            new_callable=AsyncMock,
            return_value=None,
        ):
            result = await env.execute(handle, dispatch, config)

        assert result.outcome == Outcome.SUCCESS
        assert result.signal == "all-done"

    async def test_file_signal_takes_precedence_over_stdout(self, env_kit):
        """Signal from .agent-status file wins over stdout."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="pushed",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = env_kit["config"]

        env_kit["runner"].run.return_value = _make_agent_result(
            signal_content="do-task-status: complete",
            stdout="do-task-status: all-done\n",
        )

        with patch(
            f"{_SSH_ENV}.collect_token_usage",
            new_callable=AsyncMock,
            return_value=None,
        ):
            result = await env.execute(handle, dispatch, config)

        assert result.outcome == Outcome.SUCCESS
        assert result.signal == "complete"

    async def test_nudge_recovery_on_no_signal_exit_zero(self, env_kit):
        """Nudge recovery sends follow-up when exit 0 but no signal."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="pushed",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = env_kit["config"]

        # First call: no signal at all (empty file, no stdout match)
        no_signal = _make_agent_result(
            signal_content="",
            stdout="task done, no signal",
            exit_code=0,
        )
        # Second call (nudge): agent writes signal
        nudge_ok = _make_agent_result(
            signal_content="do-task-status: complete",
            stdout="",
            exit_code=0,
        )
        env_kit["runner"].run.side_effect = [no_signal, nudge_ok]

        with patch(
            f"{_SSH_ENV}.collect_token_usage",
            new_callable=AsyncMock,
            return_value=None,
        ):
            result = await env.execute(handle, dispatch, config)

        assert result.outcome == Outcome.SUCCESS
        assert result.signal == "complete"
        # Runner called twice: original + nudge
        assert env_kit["runner"].run.await_count == 2

    async def test_nudge_recovery_falls_through_on_failure(self, env_kit):
        """If nudge also produces no signal, falls through to blind retry."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="ok",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = env_kit["config"]

        # All calls: no signal
        no_signal = _make_agent_result(
            signal_content="",
            stdout="no signal here",
            exit_code=0,
        )
        env_kit["runner"].run.return_value = no_signal

        with patch(
            f"{_SSH_ENV}.collect_token_usage",
            new_callable=AsyncMock,
            return_value=None,
        ):
            result = await env.execute(handle, dispatch, config)

        assert result.outcome == Outcome.ERROR
        # Runner called 3 times: original + nudge + blind retry
        assert env_kit["runner"].run.await_count == 3


class TestGetAccessInfo:
    async def test_returns_ssh_and_vscode_strings(self, env_kit):
        """get_access_info() returns SSH and VS Code connection strings."""
        env = env_kit["env"]
        handle = _make_handle()
        info = await env.get_access_info(handle)

        assert isinstance(info, AccessInfo)
        assert info.ssh == "ssh dev@10.0.0.42"
        assert "vscode://vscode-remote/ssh-remote+dev@10.0.0.42" in info.vscode
        assert "/workspace/myproj" in info.vscode
        assert info.working_dir == "/workspace/myproj"
        assert info.status == "running"


class TestTeardown:
    async def test_runs_teardown_cleans_workspace_closes_ssh_releases_vm(self, env_kit):
        """teardown() runs teardown commands, cleans workspace, closes SSH,
        releases VM."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="",
            stderr="",
            timed_out=False,
        )
        vm_handle = _make_vm_handle()
        workspace = _make_workspace()
        handle = _make_handle(conn=conn, vm_handle=vm_handle, workspace=workspace)

        await env.teardown(handle)

        # Teardown commands executed
        assert conn.run.await_count >= 1
        teardown_call = conn.run.call_args_list[0]
        assert "make clean" in teardown_call.args[0]

        # Workspace cleaned
        env_kit["workspace_mgr"].cleanup.assert_awaited_once_with(conn, workspace)

        # SSH closed
        conn.close.assert_awaited_once()

        # VM released
        env_kit["vm_provisioner"].release.assert_awaited_once_with(vm_handle)

        # State store records release
        env_kit["state_store"].record_release.assert_awaited_once_with("vm-abc-123")

    async def test_releases_vm_even_when_teardown_commands_fail(self, env_kit):
        """teardown() releases VM even when teardown commands fail."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.side_effect = RuntimeError("teardown cmd failed")
        vm_handle = _make_vm_handle()
        handle = _make_handle(conn=conn, vm_handle=vm_handle)

        await env.teardown(handle)

        # VM still released despite teardown command failure
        env_kit["vm_provisioner"].release.assert_awaited_once_with(vm_handle)
        env_kit["state_store"].record_release.assert_awaited_once_with("vm-abc-123")

    async def test_releases_vm_even_when_workspace_cleanup_fails(self, env_kit):
        """teardown() releases VM even when workspace cleanup fails."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="",
            stderr="",
            timed_out=False,
        )
        vm_handle = _make_vm_handle()
        handle = _make_handle(conn=conn, vm_handle=vm_handle)
        env_kit["workspace_mgr"].cleanup.side_effect = RuntimeError("cleanup boom")

        await env.teardown(handle)

        # VM still released despite cleanup failure
        env_kit["vm_provisioner"].release.assert_awaited_once_with(vm_handle)
        env_kit["state_store"].record_release.assert_awaited_once_with("vm-abc-123")

    async def test_teardown_records_release_when_provider_release_raises(self, env_kit):
        """record_release is still called when provider release raises during teardown."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="",
            stderr="",
            timed_out=False,
        )
        vm_handle = _make_vm_handle()
        handle = _make_handle(conn=conn, vm_handle=vm_handle)
        env_kit["vm_provisioner"].release.side_effect = RuntimeError("provider boom")

        await env.teardown(handle)

        env_kit["vm_provisioner"].release.assert_awaited_once_with(vm_handle)
        env_kit["state_store"].record_release.assert_awaited_once_with("vm-abc-123")

    async def test_releases_vm_even_when_ssh_close_fails(self, env_kit):
        """teardown() releases VM even when SSH close fails."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="",
            stderr="",
            timed_out=False,
        )
        conn.close.side_effect = RuntimeError("ssh close boom")
        vm_handle = _make_vm_handle()
        handle = _make_handle(conn=conn, vm_handle=vm_handle)

        await env.teardown(handle)

        # VM still released despite SSH close failure
        env_kit["vm_provisioner"].release.assert_awaited_once_with(vm_handle)
        env_kit["state_store"].record_release.assert_awaited_once_with("vm-abc-123")


class TestBuildCliCommand:
    """Test build_cli_command for each CLI type."""

    def _build(self, env_kit, cli: Cli, model: str = "sonnet", gate_cmd: str | None = None):
        from tanren_core.adapters.remote_shared import build_cli_command

        dispatch = _make_dispatch(cli=cli)
        dispatch = Dispatch(
            workflow_id=dispatch.workflow_id,
            phase=dispatch.phase,
            project=dispatch.project,
            spec_folder=dispatch.spec_folder,
            branch=dispatch.branch,
            cli=cli,
            model=model,
            gate_cmd=gate_cmd,
            context=dispatch.context,
            timeout=dispatch.timeout,
            environment_profile=dispatch.environment_profile,
            resolved_profile=DEFAULT_PROFILE,
        )
        config = env_kit["config"]
        return build_cli_command(dispatch, config)

    def test_claude_command(self, env_kit):
        cmd = self._build(env_kit, Cli.CLAUDE)
        assert "-p" in cmd
        assert "--dangerously-skip-permissions" in cmd
        assert "--model sonnet" in cmd
        assert "< .tanren-prompt.md" in cmd

    def test_opencode_command(self, env_kit):
        cmd = self._build(env_kit, Cli.OPENCODE)
        assert "opencode run" in cmd
        assert "--model sonnet" in cmd
        assert "--dir ." in cmd
        assert "-f .tanren-prompt.md" in cmd
        assert "Read the attached file" in cmd

    def test_codex_command(self, env_kit):
        cmd = self._build(env_kit, Cli.CODEX)
        assert "codex exec" in cmd
        assert "--dangerously-bypass-approvals-and-sandbox" in cmd
        assert "--model sonnet" in cmd
        assert "-C ." in cmd
        assert "< .tanren-prompt.md" in cmd

    def test_bash_command_with_gate(self, env_kit):
        cmd = self._build(env_kit, Cli.BASH, gate_cmd="make check")
        assert cmd == "make check"

    def test_bash_command_without_gate(self, env_kit):
        with pytest.raises(ValueError, match="requires a non-empty gate_cmd"):
            self._build(env_kit, Cli.BASH, gate_cmd=None)

    def test_opencode_without_model(self, env_kit):
        cmd = self._build(env_kit, Cli.OPENCODE, model="")
        assert "--model" not in cmd
        assert "opencode run" in cmd

    def test_codex_without_model(self, env_kit):
        cmd = self._build(env_kit, Cli.CODEX, model="")
        assert "--model" not in cmd
        assert "codex exec" in cmd


class TestClassifyErrorReceivesStderr:
    async def test_classify_error_gets_stderr(self, env_kit):
        """classify_error receives actual stderr from agent_result, not empty string."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="",
            stderr="",
            timed_out=False,
        )
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch()
        config = env_kit["config"]

        error_result = _make_agent_result(
            exit_code=1,
            stdout="",
            stderr="rate limit 429",
            signal_content="",
        )
        env_kit["runner"].run.side_effect = [error_result]

        with (
            patch(f"{_SSH_ENV}.map_outcome") as mock_map,
            patch(f"{_SSH_ENV}.classify_error") as mock_classify,
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            mock_map.return_value = (Outcome.ERROR, None)
            mock_classify.return_value = ErrorClass.FATAL

            await env.execute(handle, dispatch, config)

        mock_classify.assert_called_once_with(1, "", "rate limit 429", None)


class TestProvisionWorkflowId:
    async def test_handle_contains_workflow_id(self, env_kit):
        """provision() stores workflow_id in the typed runtime context."""
        env = env_kit["env"]
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn
            handle = await env.provision(dispatch, config)

        assert handle.runtime.kind == "remote"
        assert handle.runtime.workflow_id == "wf-myproj-42-1000"


class TestAgentUser:
    async def test_provision_passes_target_home(self, env_kit):
        """provision() passes agent_user home as target_home to credential injection."""
        env = env_kit["env"]
        env._agent_user = "tanren"
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
            patch(
                "tanren_core.adapters.ssh_environment.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=["claude"],
            ) as mock_inject,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn
            await env.provision(dispatch, config)

        mock_inject.assert_awaited_once()
        _, kwargs = mock_inject.call_args
        assert kwargs["target_home"] == "/home/tanren"

    async def test_execute_wraps_push_with_su(self, env_kit):
        """execute() wraps git push with su when agent_user is set."""
        env = env_kit["env"]
        env._agent_user = "tanren"
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        config = env_kit["config"]

        env_kit["workspace_mgr"].push_command.return_value = "git push origin feature-1"

        with (
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            await env.execute(handle, dispatch, config)

        push_calls = [c for c in conn.run.call_args_list if "git push" in str(c)]
        assert len(push_calls) == 1
        assert push_calls[0].args[0].startswith("su - tanren -c ")

    async def test_execute_no_push_wrap_without_agent_user(self, env_kit):
        """execute() does NOT wrap git push when agent_user is None."""
        env = env_kit["env"]
        assert env._agent_user is None
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        config = env_kit["config"]

        env_kit["workspace_mgr"].push_command.return_value = "git push origin feature-1"

        with (
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            await env.execute(handle, dispatch, config)

        push_calls = [c for c in conn.run.call_args_list if "git push" in str(c)]
        assert len(push_calls) == 1
        assert push_calls[0].args[0] == "git push origin feature-1"

    async def test_teardown_wraps_commands_with_su(self, env_kit):
        """teardown() wraps user teardown commands with su when agent_user is set."""
        env = env_kit["env"]
        env._agent_user = "tanren"
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        teardown_calls = [c for c in conn.run.call_args_list if "make clean" in str(c)]
        assert len(teardown_calls) == 1
        assert teardown_calls[0].args[0].startswith("su - tanren -c ")

    async def test_teardown_no_wrap_without_agent_user(self, env_kit):
        """teardown() does NOT wrap teardown commands when agent_user is None."""
        env = env_kit["env"]
        assert env._agent_user is None
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)

        await env.teardown(handle)

        teardown_calls = [c for c in conn.run.call_args_list if "make clean" in str(c)]
        assert len(teardown_calls) == 1
        assert teardown_calls[0].args[0] == "cd /workspace/myproj && make clean"

    async def test_execute_passes_agent_user_to_ccusage_runner(self, env_kit):
        """execute() passes agent_user as run_as_user to RemoteCommandRunner."""
        env = env_kit["env"]
        env._agent_user = "tanren"
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.CLAUDE)
        config = env_kit["config"]

        with (
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_SSH_ENV}.RemoteCommandRunner") as MockRunner,
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
        ):
            await env.execute(handle, dispatch, config)

        MockRunner.assert_called_once_with(conn, run_as_user="tanren")

    async def test_teardown_uses_absolute_paths_for_credential_cleanup(self, env_kit):
        """teardown() uses /home/tanren paths (not tilde) when agent_user is set."""
        env = env_kit["env"]
        env._agent_user = "tanren"
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(
            exit_code=0,
            stdout="",
            stderr="",
            timed_out=False,
        )
        vm_handle = _make_vm_handle()
        workspace = _make_workspace()
        handle = _make_handle(conn=conn, vm_handle=vm_handle, workspace=workspace)

        await env.teardown(handle)

        # Check that credential cleanup uses absolute paths
        rm_calls = [
            call.args[0] for call in conn.run.call_args_list if call.args[0].startswith("rm -f ")
        ]
        for rm_cmd in rm_calls:
            path = rm_cmd.replace("rm -f ", "")
            assert path.startswith("/home/tanren"), f"Expected absolute path, got: {path}"
            assert "~" not in path


class TestClose:
    async def test_close_closes_state_store(self, env_kit):
        """close() delegates to _state_store.close()."""
        env = env_kit["env"]
        await env.close()
        env_kit["state_store"].close.assert_awaited_once()


class TestSSHReadyTimeoutConfigurable:
    def test_default_timeout_is_300(self, env_kit):
        env = env_kit["env"]
        assert env._ssh_ready_timeout_secs == 300

    def test_custom_timeout_stored(self):
        env = SSHExecutionEnvironment(
            vm_provisioner=AsyncMock(),
            bootstrapper=AsyncMock(),
            workspace_mgr=AsyncMock(),
            runner=AsyncMock(),
            state_store=AsyncMock(),
            secret_loader=MagicMock(),
            ssh_config_defaults=SSHConfig(host="placeholder"),
            repo_urls={},
            ssh_ready_timeout_secs=600,
        )
        assert env._ssh_ready_timeout_secs == 600

    async def test_provision_passes_configured_timeout(self, env_kit):
        """provision() passes _ssh_ready_timeout_secs to _await_ssh_ready."""
        env = env_kit["env"]
        env._ssh_ready_timeout_secs = 450
        dispatch = _make_dispatch(resolved_profile=_make_profile())
        config = env_kit["config"]

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock) as mock_await,
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn
            await env.provision(dispatch, config)

        mock_await.assert_awaited_once()
        _, kwargs = mock_await.call_args
        assert kwargs["timeout_secs"] == 450


class TestAwaitSshReady:
    async def test_returns_immediately_when_ready(self, env_kit):
        """First check_connection() = True -> no sleep, immediate return."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.check_connection.return_value = True

        with patch(f"{_SSH_ENV}.asyncio.sleep", new_callable=AsyncMock) as mock_sleep:
            await env._await_ssh_ready(conn)

        conn.check_connection.assert_awaited_once()
        mock_sleep.assert_not_awaited()

    async def test_polls_until_ready(self, env_kit):
        """False x2 then True -> 2 sleeps, success."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.check_connection.side_effect = [False, False, True]

        with patch(f"{_SSH_ENV}.asyncio.sleep", new_callable=AsyncMock) as mock_sleep:
            await env._await_ssh_ready(conn)

        assert conn.check_connection.await_count == 3
        assert mock_sleep.await_count == 2

    async def test_raises_timeout_when_never_ready(self, env_kit):
        """Always False -> TimeoutError raised."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.check_connection.return_value = False
        conn.get_host_identifier = MagicMock(return_value="dev@10.0.0.42:22")

        # Advance monotonic clock by 50s per call: 0, 50, 100, 150...
        # With timeout=120, the loop runs ~2 iterations then exits
        clock = iter(range(0, 1000, 50))

        with (
            patch(f"{_SSH_ENV}.asyncio.sleep", new_callable=AsyncMock),
            patch(f"{_SSH_ENV}.time.monotonic", side_effect=lambda: next(clock)),
            pytest.raises(TimeoutError, match="SSH not reachable"),
        ):
            await env._await_ssh_ready(conn, timeout_secs=120)


class TestMcpInjection:
    async def test_provision_calls_inject_mcp_config(self, env_kit):
        """Provision with mcp servers calls inject_mcp_config with servers."""
        from tanren_core.env.environment_schema import McpServerConfig

        env = env_kit["env"]
        config = env_kit["config"]
        workspace_mgr = env_kit["workspace_mgr"]
        workspace_mgr.inject_mcp_config = AsyncMock()

        mcp_profile = EnvironmentProfile(
            name="default",
            type=EnvironmentProfileType.REMOTE,
            resources=ResourceRequirements(cpu=2, memory_gb=4, gpu=False),
            setup=("make setup",),
            teardown=("make clean",),
            mcp={
                "context7": McpServerConfig(
                    url="https://mcp.context7.com/sse",
                    headers={"Authorization": "MCP_CONTEXT7_KEY"},
                )
            },
        )
        dispatch = _make_dispatch(cli=Cli.CLAUDE, resolved_profile=mcp_profile)

        with (
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch(f"{_SSH_ENV}.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
            mock_conn.run.return_value = _OK_REMOTE_RESULT
            MockSSH.return_value = mock_conn
            await env.provision(dispatch, config)

        workspace_mgr.inject_mcp_config.assert_awaited_once()
        call_args = workspace_mgr.inject_mcp_config.call_args
        assert "context7" in call_args.args[2]
        assert call_args.args[2]["context7"].url == "https://mcp.context7.com/sse"
