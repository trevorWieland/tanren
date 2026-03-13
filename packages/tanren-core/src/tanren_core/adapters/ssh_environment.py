"""SSHExecutionEnvironment — remote VM execution via SSH."""

from __future__ import annotations

import asyncio
import logging
import shlex
import time
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import cast

import yaml
from dotenv import dotenv_values

from tanren_core.adapters.events import BootstrapCompleted, VMProvisioned, VMReleased
from tanren_core.adapters.protocols import (
    EnvironmentBootstrapper,
    EventEmitter,
    VMStateStore,
)
from tanren_core.adapters.protocols import (
    VMProvisioner as VMProvisionerProtocol,
)
from tanren_core.adapters.protocols import (
    WorkspaceManager as WorkspaceManagerProtocol,
)
from tanren_core.adapters.remote_runner import RemoteAgentRunner
from tanren_core.adapters.remote_types import (
    VMHandle,
    VMRequirements,
    WorkspaceSpec,
)
from tanren_core.adapters.ssh import SSHConfig, SSHConnection
from tanren_core.adapters.types import (
    AccessInfo,
    EnvironmentHandle,
    PhaseResult,
    RemoteEnvironmentRuntime,
)
from tanren_core.config import Config
from tanren_core.env.environment_schema import EnvironmentProfile, parse_environment_profiles
from tanren_core.errors import TRANSIENT_BACKOFF, ErrorClass, classify_error
from tanren_core.process import assemble_prompt
from tanren_core.schemas import Dispatch, Outcome, Phase
from tanren_core.secrets import SecretLoader
from tanren_core.signals import map_outcome, parse_signal_token

logger = logging.getLogger(__name__)

_PUSH_PHASES = frozenset({Phase.DO_TASK, Phase.AUDIT_TASK, Phase.RUN_DEMO, Phase.AUDIT_SPEC})

_SSH_READY_TIMEOUT_SECS = 120
_SSH_READY_POLL_SECS = 3


class SSHExecutionEnvironment:
    """ExecutionEnvironment backed by remote VM execution over SSH.

    Composes VMProvisioner, SSHConnection, EnvironmentBootstrapper,
    WorkspaceManager, RemoteAgentRunner, and VMStateStore into the
    provision/execute/teardown lifecycle.
    """

    def __init__(
        self,
        *,
        vm_provisioner: VMProvisionerProtocol,
        bootstrapper: EnvironmentBootstrapper,
        workspace_mgr: WorkspaceManagerProtocol,
        runner: RemoteAgentRunner,
        state_store: VMStateStore,
        secret_loader: SecretLoader,
        emitter: EventEmitter,
        ssh_config_defaults: SSHConfig,
        repo_urls: dict[str, str],
    ) -> None:
        """Initialize with remote execution adapters and configuration."""
        self._vm_provisioner = vm_provisioner
        self._bootstrapper = bootstrapper
        self._workspace_mgr = workspace_mgr
        self._runner = runner
        self._state_store = state_store
        self._secret_loader = secret_loader
        self._emitter = emitter
        self._ssh_defaults = ssh_config_defaults
        self._repo_urls = repo_urls

    @property
    def ssh_defaults(self) -> SSHConfig:
        """Return default SSH settings used for remote connections."""
        return self._ssh_defaults

    async def close(self) -> None:
        """Release resources held by the environment (DB connections)."""
        await self._state_store.close()

    async def provision(self, dispatch: Dispatch, config: Config) -> EnvironmentHandle:
        """Acquire VM, bootstrap, setup workspace, inject secrets.

        Returns:
            EnvironmentHandle for remote execution.

        Raises:
            RuntimeError: If no repo URL is configured for the project.
        """
        # 1. Read tanren.yml LOCALLY to get environment profile
        profile = self._resolve_profile(dispatch, config)

        # 2. Build VM requirements from profile
        requirements = VMRequirements(
            profile=dispatch.environment_profile,
            cpu=profile.resources.cpu,
            memory_gb=profile.resources.memory_gb,
            gpu=profile.resources.gpu,
            server_type=profile.server_type,
        )

        # 3. Acquire VM
        vm_handle = await self._vm_provisioner.acquire(requirements)
        conn: SSHConnection | None = None

        try:
            # 4. Create SSH connection
            ssh_config = SSHConfig(
                host=vm_handle.host,
                user=self._ssh_defaults.user,
                key_path=self._ssh_defaults.key_path,
                port=self._ssh_defaults.port,
                connect_timeout=self._ssh_defaults.connect_timeout,
                host_key_policy=self._ssh_defaults.host_key_policy,
            )
            conn = SSHConnection(ssh_config)

            # 4b. Wait for SSH to accept connections (sshd lags behind API status)
            await self._await_ssh_ready(conn)

            # 5. Bootstrap VM (idempotent)
            bootstrap_result = await self._bootstrapper.bootstrap(conn)

            await self._emitter.emit(
                BootstrapCompleted(
                    timestamp=_now(),
                    workflow_id=dispatch.workflow_id,
                    vm_id=vm_handle.vm_id,
                    installed=list(bootstrap_result.installed),
                    skipped=list(bootstrap_result.skipped),
                    duration_secs=bootstrap_result.duration_secs,
                )
            )

            # 6. Setup workspace
            repo_url = self._repo_urls.get(dispatch.project, "")
            if not repo_url:
                raise RuntimeError(f"No repo URL configured for project: {dispatch.project}")

            workspace_spec = WorkspaceSpec(
                project=dispatch.project,
                repo_url=repo_url,
                branch=dispatch.branch,
                setup_commands=profile.setup,
            )
            workspace_path = await self._workspace_mgr.setup(conn, workspace_spec)

            # 7. Inject secrets
            project_env = self._load_project_env(dispatch, config)
            bundle = self._secret_loader.build_bundle(project_env)
            await self._workspace_mgr.inject_secrets(conn, workspace_path, bundle)

            # 8. Record assignment
            await self._state_store.record_assignment(
                vm_id=vm_handle.vm_id,
                workflow_id=dispatch.workflow_id,
                project=dispatch.project,
                spec=dispatch.spec_folder,
                host=vm_handle.host,
            )

            await self._emitter.emit(
                VMProvisioned(
                    timestamp=_now(),
                    workflow_id=dispatch.workflow_id,
                    vm_id=vm_handle.vm_id,
                    host=vm_handle.host,
                    provider=vm_handle.provider,
                    project=dispatch.project,
                    profile=dispatch.environment_profile,
                    hourly_cost=vm_handle.hourly_cost,
                )
            )

            # 9. Return handle
            return EnvironmentHandle(
                env_id=str(uuid.uuid4()),
                worktree_path=Path(workspace_path.path),
                branch=dispatch.branch,
                project=dispatch.project,
                runtime=RemoteEnvironmentRuntime(
                    vm_handle=vm_handle,
                    connection=conn,
                    workspace_path=workspace_path,
                    profile=profile,
                    teardown_commands=profile.teardown,
                    provision_start=time.monotonic(),
                    workflow_id=dispatch.workflow_id,
                ),
            )

        except Exception:
            # Clean up on failure — no orphaned VMs
            if conn is not None:
                try:
                    await conn.close()
                except Exception:
                    logger.warning("SSH close failed during provision cleanup")
            await self._vm_provisioner.release(vm_handle)
            await self._state_store.record_release(vm_handle.vm_id)
            raise

    async def execute(
        self,
        handle: EnvironmentHandle,
        dispatch: Dispatch,
        config: Config,
        *,
        dispatch_stem: str = "",
    ) -> PhaseResult:
        """Run agent on remote VM with retry logic.

        Returns:
            PhaseResult with outcome, signal, and metrics.

        Raises:
            RuntimeError: If the handle does not contain a remote runtime.
        """
        if handle.runtime.kind != "remote":
            raise RuntimeError("SSHExecutionEnvironment requires remote runtime handle")
        remote_runtime = cast(RemoteEnvironmentRuntime, handle.runtime)
        conn = cast(SSHConnection, remote_runtime.connection)
        workspace = remote_runtime.workspace_path

        start = time.monotonic()
        transient_retries = 0

        # Build full prompt (same as local path)
        command_name = dispatch.phase.value
        command_file = handle.worktree_path / config.commands_dir / f"{command_name}.md"
        prompt_content = assemble_prompt(
            command_file, dispatch.spec_folder, command_name, dispatch.context
        )

        while True:
            # Build CLI command
            cli_command = self._build_cli_command(dispatch, config)
            signal_path = f"{workspace.path}/{dispatch.spec_folder}/.agent-status"

            # Run agent
            agent_result = await self._runner.run(
                conn,
                workspace,
                prompt_content=prompt_content,
                cli_command=cli_command,
                signal_path=signal_path,
                timeout=dispatch.timeout,
            )

            # Parse signal token from raw file content
            raw_signal = agent_result.signal_content or ""
            signal_token = (
                parse_signal_token(command_name, raw_signal) if raw_signal.strip() else None
            )

            # Map outcome
            outcome, signal_val = map_outcome(
                dispatch.phase,
                signal_token,
                agent_result.exit_code,
                agent_result.timed_out,
            )

            # Retry on transient errors
            if outcome in (Outcome.ERROR, Outcome.TIMEOUT):
                error_class = classify_error(
                    agent_result.exit_code,
                    agent_result.stdout,
                    agent_result.stderr,
                    signal_val,
                )
                if error_class == ErrorClass.TRANSIENT and transient_retries < 3:
                    transient_retries += 1
                    backoff = TRANSIENT_BACKOFF[transient_retries - 1]
                    logger.warning(
                        "Transient error (attempt %d/3), retrying in %ds",
                        transient_retries,
                        backoff,
                    )
                    await asyncio.sleep(backoff)
                    continue
                if error_class == ErrorClass.AMBIGUOUS and transient_retries < 1:
                    transient_retries += 1
                    logger.warning("Ambiguous error, retrying once in 10s")
                    await asyncio.sleep(10)
                    continue

            break

        duration = int(time.monotonic() - start)

        # Remote postflight: push on push phases
        final_stdout = agent_result.stdout
        if dispatch.phase in _PUSH_PHASES and outcome not in (Outcome.ERROR, Outcome.TIMEOUT):
            push_cmd = self._workspace_mgr.push_command(workspace.path, dispatch.branch)
            push_result = await conn.run(push_cmd, timeout=120)
            if push_result.exit_code != 0:
                logger.error(
                    "Remote git push failed (exit %d) for %s branch %s: %s",
                    push_result.exit_code,
                    dispatch.project,
                    dispatch.branch,
                    push_result.stderr,
                )
                push_diag = (
                    "\n[worker] Remote git push failed during postflight.\n"
                    f"[worker] exit_code={push_result.exit_code}\n"
                    f"[worker] stderr: {push_result.stderr}\n"
                )
                final_stdout = (final_stdout or "") + push_diag

        return PhaseResult(
            outcome=outcome,
            signal=signal_val,
            exit_code=agent_result.exit_code,
            stdout=final_stdout,
            duration_secs=duration,
            preflight_passed=True,
            postflight_result=None,
            env_report=None,
            gate_output=None,
            unchecked_tasks=0,
            plan_hash="00000000",
            retries=transient_retries,
        )

    async def get_access_info(self, handle: EnvironmentHandle) -> AccessInfo:
        """Return SSH and VS Code connection info.

        Raises:
            RuntimeError: If the handle does not contain a remote runtime.
        """
        if handle.runtime.kind != "remote":
            raise RuntimeError("SSHExecutionEnvironment requires remote runtime handle")
        remote_runtime = cast(RemoteEnvironmentRuntime, handle.runtime)
        vm_handle = remote_runtime.vm_handle
        ssh_str = f"ssh {self._ssh_defaults.user}@{vm_handle.host}"
        vscode_str = (
            f"vscode://vscode-remote/ssh-remote+"
            f"{self._ssh_defaults.user}@{vm_handle.host}"
            f"{handle.worktree_path}"
        )
        return AccessInfo(
            ssh=ssh_str,
            vscode=vscode_str,
            working_dir=str(handle.worktree_path),
            status="running",
        )

    async def release_vm(self, vm_handle: VMHandle) -> None:
        """Release a VM through the provisioner without full teardown."""
        await self._vm_provisioner.release(vm_handle)

    async def teardown(self, handle: EnvironmentHandle) -> None:
        """Guaranteed VM release with try/finally at every step.

        Raises:
            RuntimeError: If the handle does not contain a remote runtime.
        """
        if handle.runtime.kind != "remote":
            raise RuntimeError("SSHExecutionEnvironment requires remote runtime handle")
        remote_runtime = cast(RemoteEnvironmentRuntime, handle.runtime)
        conn = cast(SSHConnection, remote_runtime.connection)
        workspace = remote_runtime.workspace_path
        teardown_cmds = remote_runtime.teardown_commands
        vm_handle = remote_runtime.vm_handle
        provision_start = remote_runtime.provision_start

        try:
            for cmd in teardown_cmds:
                try:
                    await conn.run(
                        f"cd {shlex.quote(workspace.path)} && {cmd}",
                        timeout=120,
                    )
                except Exception:
                    logger.warning("Teardown command failed: %s", cmd)
        finally:
            try:
                await self._workspace_mgr.cleanup(conn, workspace)
            except Exception:
                logger.warning("Workspace cleanup failed")
            finally:
                try:
                    await conn.close()
                except Exception:
                    logger.warning("SSH close failed")
                finally:
                    # MUST happen — no orphaned VMs
                    await self._vm_provisioner.release(vm_handle)
                    await self._state_store.record_release(vm_handle.vm_id)

                    duration = int(time.monotonic() - provision_start)
                    cost = None
                    if vm_handle.hourly_cost is not None:
                        cost = vm_handle.hourly_cost * (duration / 3600)

                    await self._emitter.emit(
                        VMReleased(
                            timestamp=_now(),
                            workflow_id=remote_runtime.workflow_id,
                            vm_id=vm_handle.vm_id,
                            duration_secs=duration,
                            estimated_cost=cost,
                        )
                    )

    async def _await_ssh_ready(
        self,
        conn: SSHConnection,
        *,
        timeout: int = _SSH_READY_TIMEOUT_SECS,  # noqa: ASYNC109
    ) -> None:
        """Poll SSH until the host accepts connections or deadline expires.

        Raises:
            TimeoutError: If SSH is not reachable within the timeout.
        """
        deadline = time.monotonic() + timeout
        attempt = 0
        while time.monotonic() < deadline:
            attempt += 1
            if await conn.check_connection():
                logger.info("SSH ready after %d attempt(s)", attempt)
                return
            logger.debug(
                "SSH not ready (attempt %d), retrying in %ds",
                attempt,
                _SSH_READY_POLL_SECS,
            )
            await asyncio.sleep(_SSH_READY_POLL_SECS)
        raise TimeoutError(f"SSH not reachable within {timeout}s on {conn.get_host_identifier()}")

    def _resolve_profile(self, dispatch: Dispatch, config: Config) -> EnvironmentProfile:
        """Read tanren.yml locally and resolve environment profile.

        Returns:
            Resolved EnvironmentProfile, falling back to default.
        """
        tanren_yml = Path(config.github_dir) / dispatch.project / "tanren.yml"
        if tanren_yml.exists():
            with open(tanren_yml) as f:
                loaded = yaml.safe_load(f) or {}
            data = loaded if isinstance(loaded, dict) else {}
            profiles = parse_environment_profiles(data)
        else:
            profiles = parse_environment_profiles({})

        profile = profiles.get(dispatch.environment_profile)
        if profile is None:
            logger.warning(
                "Profile %s not found, using default",
                dispatch.environment_profile,
            )
            profile = profiles.get("default", EnvironmentProfile(name="default"))

        return profile

    def _load_project_env(self, dispatch: Dispatch, config: Config) -> dict[str, str]:
        """Load project .env file locally for secret injection.

        Returns:
            Dict of env var key-value pairs from the project .env file.
        """
        env_file = Path(config.github_dir) / dispatch.project / ".env"
        if not env_file.exists():
            return {}
        values = dotenv_values(env_file)
        return {k: v for k, v in values.items() if v is not None}

    def _build_cli_command(self, dispatch: Dispatch, config: Config) -> str:
        """Build the CLI command string for remote execution.

        Returns:
            Shell command string for the agent CLI.

        Raises:
            ValueError: If the CLI type is unsupported or gate_cmd is empty for bash.
        """
        if dispatch.cli.value == "claude":
            cmd = config.claude_path
            cmd += " -p --dangerously-skip-permissions"
            if dispatch.model:
                cmd += f" --model {shlex.quote(dispatch.model)}"
            cmd += " < .tanren-prompt.md"
            return cmd
        if dispatch.cli.value == "bash":
            gate_cmd = (dispatch.gate_cmd or "").strip()
            if gate_cmd:
                return gate_cmd
            raise ValueError("Gate dispatch requires a non-empty gate_cmd when cli=bash")
        if dispatch.cli.value == "opencode":
            cmd = config.opencode_path
            cmd += " run"
            if dispatch.model:
                cmd += f" --model {shlex.quote(dispatch.model)}"
            cmd += " --dir ."
            cmd += ' "Read the attached file and follow its instructions exactly."'
            cmd += " -f .tanren-prompt.md"
            return cmd
        if dispatch.cli.value == "codex":
            cmd = config.codex_path
            cmd += " exec --dangerously-bypass-approvals-and-sandbox"
            if dispatch.model:
                cmd += f" --model {shlex.quote(dispatch.model)}"
            cmd += " -C ."
            cmd += " < .tanren-prompt.md"
            return cmd
        raise ValueError(f"Unsupported CLI for remote execution: {dispatch.cli.value}")


def _now() -> str:
    return datetime.now(UTC).isoformat()
