"""Tests for SSHExecutionEnvironment composition layer."""

from __future__ import annotations

import time
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

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
    ProvisionError,
    RemoteEnvironmentRuntime,
)
from tanren_core.config import Config
from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    EnvironmentProfileType,
    ResourceRequirements,
)
from tanren_core.errors import ErrorClass
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase

_SSH_ENV = "tanren_core.adapters.ssh_environment"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_config(tmp_path: Path) -> Config:
    return Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        roles_config_path=str(tmp_path / "roles.yml"),
    )


def _make_dispatch(
    phase: Phase = Phase.DO_TASK,
    cli: Cli = Cli.CLAUDE,
    project: str = "myproj",
    branch: str = "feature-1",
    context: str = "Do the work",
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


@pytest.fixture()
def env_kit(tmp_path: Path):
    """Build an SSHExecutionEnvironment with all sub-adapters mocked."""
    vm_provisioner = AsyncMock()
    bootstrapper = AsyncMock()
    workspace_mgr = AsyncMock()
    runner = AsyncMock()
    state_store = AsyncMock()
    secret_loader = MagicMock()
    emitter = AsyncMock()

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
        emitter=emitter,
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
        "emitter": emitter,
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
        from tanren_core.adapters.remote_types import VMAssignment  # noqa: PLC0415

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
        from tanren_core.adapters.remote_types import VMAssignment  # noqa: PLC0415

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
        from tanren_core.adapters.remote_types import VMAssignment  # noqa: PLC0415

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
        dispatch = _make_dispatch()
        config = env_kit["config"]

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
            patch.object(env, "_load_project_env", return_value={"KEY": "val"}),
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            mock_conn = AsyncMock()
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

        # Workspace setup called
        env_kit["workspace_mgr"].setup.assert_awaited_once()

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
        dispatch = _make_dispatch()
        config = env_kit["config"]

        import asyncio  # noqa: PLC0415

        # Make bootstrap raise CancelledError (simulates task cancellation)
        env_kit["bootstrapper"].bootstrap.side_effect = asyncio.CancelledError()

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
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
        dispatch = _make_dispatch()
        config = env_kit["config"]

        # Make bootstrap fail
        env_kit["bootstrapper"].bootstrap.side_effect = RuntimeError("bootstrap boom")

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
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
        dispatch = _make_dispatch()
        config = env_kit["config"]

        # Make bootstrap fail (triggering cleanup)
        env_kit["bootstrapper"].bootstrap.side_effect = RuntimeError("bootstrap boom")
        # Make release also fail
        env_kit["vm_provisioner"].release.side_effect = RuntimeError("provider down")

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
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
        dispatch = _make_dispatch(project="unknown-project")
        config = env_kit["config"]

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
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
        dispatch = _make_dispatch()
        config = env_kit["config"]
        call_order: list[str] = []

        async def _track_ssh_ready(conn, **kwargs):  # noqa: RUF029
            call_order.append("ssh_ready")

        async def _track_bootstrap(conn):  # noqa: RUF029
            call_order.append("bootstrap")
            return _make_bootstrap_result()

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
            patch.object(env, "_load_project_env", return_value={}),
            patch.object(env, "_await_ssh_ready", side_effect=_track_ssh_ready),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            MockSSH.return_value = AsyncMock()
            env_kit["bootstrapper"].bootstrap.side_effect = _track_bootstrap
            await env.provision(dispatch, config)

        assert call_order == ["ssh_ready", "bootstrap"]

    async def test_releases_vm_on_ssh_timeout(self, env_kit):
        """provision() releases VM when SSH never becomes reachable."""
        env = env_kit["env"]
        dispatch = _make_dispatch()
        config = env_kit["config"]

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
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

    async def test_unknown_profile_raises_provision_error(self, env_kit, tmp_path):
        """Missing environment profile raises ProvisionError (not ValueError),
        so the manager emits a user-facing error instead of an internal failure."""
        env = env_kit["env"]
        config = _make_config(tmp_path)
        dispatch = _make_dispatch()
        dispatch = dispatch.model_copy(update={"environment_profile": "nonexistent"})

        # Write a tanren.yml with only the default profile
        project_dir = tmp_path / dispatch.project
        project_dir.mkdir(exist_ok=True)
        (project_dir / "tanren.yml").write_text("environments: {}")

        with pytest.raises(ProvisionError) as exc_info:
            await env.provision(dispatch, config)

        result = exc_info.value.result
        assert result.outcome == Outcome.ERROR
        assert result.workflow_id == dispatch.workflow_id
        assert result.phase == dispatch.phase
        assert "nonexistent" in (result.tail_output or "")

        # No VM should have been acquired
        env_kit["vm_provisioner"].acquire.assert_not_awaited()


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
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
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
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
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
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
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
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
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
        from tanren_core.ccusage import TokenUsage  # noqa: PLC0415

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
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(
                f"{_SSH_ENV}.collect_token_usage",
                new_callable=AsyncMock,
                return_value=mock_usage,
            ),
        ):
            result = await env.execute(handle, dispatch, config)

        assert result.token_usage is not None
        assert result.token_usage["total_cost"] == pytest.approx(1.50)
        assert result.token_usage["total_tokens"] == 300

    async def test_execute_skips_token_usage_for_bash(self, env_kit):
        """execute() does not collect token usage for Cli.BASH dispatches."""
        env = env_kit["env"]
        conn = AsyncMock()
        conn.run.return_value = RemoteResult(exit_code=0, stdout="ok", stderr="", timed_out=False)
        handle = _make_handle(conn=conn)
        dispatch = _make_dispatch(cli=Cli.BASH)
        config = env_kit["config"]

        with (
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
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
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
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
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
            patch(f"{_SSH_ENV}.map_outcome", return_value=(Outcome.SUCCESS, "success")),
            patch(f"{_SSH_ENV}.collect_token_usage", new_callable=AsyncMock, return_value=None),
            patch(f"{_SSH_ENV}.inject_all_cli_credentials", new_callable=AsyncMock) as mock_inject,
        ):
            await env.execute(handle, dispatch, config)

        mock_inject.assert_not_awaited()


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
    """Test _build_cli_command for each CLI type."""

    def _build(self, env_kit, cli: Cli, model: str = "sonnet", gate_cmd: str | None = None):
        env = env_kit["env"]
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
        )
        config = env_kit["config"]
        return env._build_cli_command(dispatch, config)

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
            patch(f"{_SSH_ENV}.assemble_prompt", return_value="prompt"),
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
        dispatch = _make_dispatch()
        config = env_kit["config"]

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
            patch.object(env, "_load_project_env", return_value={}),
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            MockSSH.return_value = AsyncMock()
            handle = await env.provision(dispatch, config)

        assert handle.runtime.kind == "remote"
        assert handle.runtime.workflow_id == "wf-myproj-42-1000"


class TestAgentUser:
    async def test_provision_passes_target_home(self, env_kit):
        """provision() passes agent_user home as target_home to credential injection."""
        env = env_kit["env"]
        env._agent_user = "tanren"
        dispatch = _make_dispatch()
        config = env_kit["config"]

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
            patch.object(env, "_load_project_env", return_value={}),
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock),
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
            patch(
                "tanren_core.adapters.ssh_environment.inject_all_cli_credentials",
                new_callable=AsyncMock,
                return_value=["claude"],
            ) as mock_inject,
        ):
            MockSSH.return_value = AsyncMock()
            await env.provision(dispatch, config)

        mock_inject.assert_awaited_once()
        _, kwargs = mock_inject.call_args
        assert kwargs["target_home"] == "/home/tanren"

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
            emitter=AsyncMock(),
            ssh_config_defaults=SSHConfig(host="placeholder"),
            repo_urls={},
            ssh_ready_timeout_secs=600,
        )
        assert env._ssh_ready_timeout_secs == 600

    async def test_provision_passes_configured_timeout(self, env_kit):
        """provision() passes _ssh_ready_timeout_secs to _await_ssh_ready."""
        env = env_kit["env"]
        env._ssh_ready_timeout_secs = 450
        dispatch = _make_dispatch()
        config = env_kit["config"]

        with (
            patch.object(env, "_resolve_profile", return_value=_make_profile()),
            patch.object(env, "_load_project_env", return_value={}),
            patch.object(env, "_await_ssh_ready", new_callable=AsyncMock) as mock_await,
            patch("tanren_core.adapters.ssh_environment.SSHConnection") as MockSSH,
        ):
            MockSSH.return_value = AsyncMock()
            await env.provision(dispatch, config)

        mock_await.assert_awaited_once()
        _, kwargs = mock_await.call_args
        assert kwargs["timeout"] == 450


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
            await env._await_ssh_ready(conn, timeout=120)
