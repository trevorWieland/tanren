"""SSHExecutionEnvironment — remote VM execution via SSH."""

from __future__ import annotations

import asyncio
import logging
import os
import shlex
import time
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING, cast

from tanren_core.adapters.credentials import (
    DEFAULT_CREDENTIAL_PROVIDERS,
    CredentialProvider,
    all_credential_cleanup_paths,
    inject_all_cli_credentials,
)
from tanren_core.adapters.remote_shared import (
    CLI_AUTH_GROUPS,
    PUSH_PHASES,
    build_cli_command,
    extract_signal_token,
    validate_cli_auth,
    wrap_for_agent_user,
)
from tanren_core.adapters.remote_types import (
    DryRunInfo,
    VMHandle,
    VMProvider,
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
from tanren_core.ccusage import RemoteCommandRunner, collect_token_usage
from tanren_core.errors import TRANSIENT_BACKOFF, ErrorClass, classify_error
from tanren_core.postflight import PostflightResult
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase
from tanren_core.signals import map_outcome

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import (
        EnvironmentBootstrapper,
        VMStateStore,
    )
    from tanren_core.adapters.protocols import (
        VMProvisioner as VMProvisionerProtocol,
    )
    from tanren_core.adapters.protocols import (
        WorkspaceManager as WorkspaceManagerProtocol,
    )
    from tanren_core.adapters.remote_runner import RemoteAgentRunner
    from tanren_core.secrets import SecretLoader
    from tanren_core.worker_config import WorkerConfig

logger = logging.getLogger(__name__)

_SSH_READY_TIMEOUT_SECS = 300
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
        ssh_config_defaults: SSHConfig,
        repo_urls: dict[str, str],
        provider: VMProvider = VMProvider.MANUAL,
        ssh_ready_timeout_secs: int = _SSH_READY_TIMEOUT_SECS,
        credential_providers: tuple[CredentialProvider, ...] = DEFAULT_CREDENTIAL_PROVIDERS,
        agent_user: str | None = None,
    ) -> None:
        """Initialize with remote execution adapters and configuration."""
        self._vm_provisioner = vm_provisioner
        self._bootstrapper = bootstrapper
        self._workspace_mgr = workspace_mgr
        self._runner = runner
        self._state_store = state_store
        self._secret_loader = secret_loader
        self._ssh_defaults = ssh_config_defaults
        self._repo_urls = repo_urls
        self._provider = provider
        self._ssh_ready_timeout_secs = ssh_ready_timeout_secs
        self._credential_providers = credential_providers
        self._agent_user = agent_user

    @property
    def ssh_defaults(self) -> SSHConfig:
        """Return default SSH settings used for remote connections."""
        return self._ssh_defaults

    async def close(self) -> None:
        """Release resources held by the environment (DB connections)."""
        await self._state_store.close()

    async def recover_stale_assignments(self) -> int:
        """Release any unreleased VM assignments left by a prior crash.

        Returns:
            Number of recovered assignments.
        """
        assignments = await self._state_store.get_active_assignments()
        if not assignments:
            return 0

        logger.info("Recovering %d stale VM assignment(s)...", len(assignments))

        for a in assignments:
            stale_handle = VMHandle(
                vm_id=a.vm_id,
                host=a.host,
                provider=self._provider,
                created_at=a.assigned_at,
            )
            try:
                await self._vm_provisioner.release(stale_handle)
            except Exception:
                logger.warning(
                    "Failed provider release for stale VM %s (%s)",
                    a.vm_id,
                    a.host,
                    exc_info=True,
                )
            finally:
                await self._state_store.record_release(a.vm_id)
                logger.info("Recovered stale VM %s (%s)", a.vm_id, a.host)

        return len(assignments)

    async def dry_run(
        self,
        requirements: VMRequirements,  # noqa: ARG002 — requirements used by provisioner internally via self._vm_provisioner
    ) -> DryRunInfo:
        """Dry-run provision — return what would happen without creating resources.

        Returns:
            DryRunInfo with provider info from this environment.
        """
        return DryRunInfo(
            provider=self._provider,
            would_provision=True,
        )

    async def provision(
        self,
        dispatch: Dispatch,
        config: WorkerConfig,  # noqa: ARG002 — required by protocol; config no longer read here
    ) -> EnvironmentHandle:
        """Acquire VM, bootstrap, setup workspace, inject secrets.

        Returns:
            EnvironmentHandle for remote execution.

        Raises:
            RuntimeError: If no repo URL is configured for the project.
        """
        # 1. Use dispatch-carried profile (resolved by CLI/API before dispatch)
        profile = dispatch.resolved_profile

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
                key_content_env=self._ssh_defaults.key_content_env,
                port=self._ssh_defaults.port,
                connect_timeout=self._ssh_defaults.connect_timeout,
                host_key_policy=self._ssh_defaults.host_key_policy,
            )
            conn = SSHConnection(ssh_config)

            # 4b. Wait for SSH to accept connections (sshd lags behind API status)
            await self._await_ssh_ready(conn, timeout_secs=self._ssh_ready_timeout_secs)

            # 5. Bootstrap VM (idempotent)
            await self._bootstrapper.bootstrap(conn)

            # 6. Setup workspace — repo URL from dispatch profile or instance mapping
            repo_url = ""
            if profile.remote_config is not None:
                repo_url = profile.remote_config.repo_url
            if not repo_url:
                repo_url = self._repo_urls.get(dispatch.project, "")
            if not repo_url:
                raise RuntimeError(f"No repo URL configured for project: {dispatch.project}")

            # Clone only (setup commands run later as agent user)
            workspace_spec = WorkspaceSpec(
                project=dispatch.project,
                repo_url=repo_url,
                branch=dispatch.branch,
                setup_commands=(),  # deferred to after chown
            )
            workspace_path = await self._workspace_mgr.setup(conn, workspace_spec)

            # 7a. Inject secrets
            # Resolve required_secrets from daemon's os.environ (reference-based)
            developer_overrides: dict[str, str] | None = None
            if dispatch.required_secrets:
                resolved: dict[str, str] = {}
                missing: list[str] = []
                for name in dispatch.required_secrets:
                    value = os.environ.get(name, "")
                    if value:
                        resolved[name] = value
                    else:
                        missing.append(name)
                if missing:
                    # Determine which missing secrets are non-auth (truly required)
                    # vs auth alternatives (handled by validate_cli_auth's group logic)
                    auth_names: set[str] = set()
                    for groups in CLI_AUTH_GROUPS.values():
                        for group in groups:
                            auth_names.update(group)
                    non_auth_missing = [n for n in missing if n not in auth_names]
                    if non_auth_missing:
                        raise RuntimeError(
                            f"Required secrets not found in daemon environment: "
                            f"{', '.join(non_auth_missing)}. "
                            f"Set these in the daemon's environment or secrets.env."
                        )
                    if missing:
                        logger.info(
                            "Auth secrets not in daemon env (alternative may suffice): %s",
                            ", ".join(n for n in missing if n in auth_names),
                        )
                # Validate CLI auth: at least one secret in each auth
                # group must be resolvable for the dispatch's CLI.
                validate_cli_auth(dispatch.cli, resolved, phase=dispatch.phase.value)
                developer_overrides = resolved

            project_env = dispatch.project_env
            cloud_secrets = dispatch.cloud_secrets or None
            bundle = self._secret_loader.build_bundle(
                project_env,
                cloud_secrets=cloud_secrets,
                developer_overrides=developer_overrides,
            )
            await self._workspace_mgr.inject_secrets(conn, workspace_path, bundle)

            # 7b. Inject MCP config
            if profile.mcp:
                await self._workspace_mgr.inject_mcp_config(conn, workspace_path, profile.mcp)

            # 7c. Transfer workspace ownership to agent user BEFORE setup
            if self._agent_user:
                quoted_user = shlex.quote(self._agent_user)
                quoted_ws = shlex.quote(workspace_path.path)
                await conn.run(
                    f"chown {quoted_user}:{quoted_user}"
                    f" /workspace/.developer-secrets /workspace/.git-askpass 2>/dev/null;"
                    f" chown -R {quoted_user}:{quoted_user} {quoted_ws}",
                    timeout_secs=30,
                )

            # 7d. Run setup commands AS agent user (so uv/pip create venvs with correct ownership)
            if profile.setup:
                quoted_ws = shlex.quote(workspace_path.path)
                for cmd in profile.setup:
                    logger.info(
                        "Running setup command (as %s): %s", self._agent_user or "root", cmd
                    )
                    setup_cmd = f"cd {quoted_ws} && {cmd}"
                    if self._agent_user:
                        setup_cmd = (
                            f"su - {shlex.quote(self._agent_user)} -c {shlex.quote(setup_cmd)}"
                        )
                    result = await conn.run(setup_cmd, timeout_secs=300)
                    if result.exit_code != 0:
                        raise RuntimeError(f"Setup command failed ({cmd}): {result.stderr}")

            # 8. Inject all CLI credentials
            target_home = f"/home/{self._agent_user}" if self._agent_user else None
            injected = await inject_all_cli_credentials(
                conn, bundle, self._credential_providers, target_home=target_home
            )
            logger.info("Injected credentials: %s", injected)

            # 8b. Ensure agent user owns their entire home directory
            # (bootstrap and credential injection run as root, leaving root-owned dirs)
            if self._agent_user:
                await conn.run(
                    f"chown -R {self._agent_user}:{self._agent_user} /home/{self._agent_user}",
                    timeout_secs=10,
                )

            # 9. Record assignment
            await self._state_store.record_assignment(
                vm_id=vm_handle.vm_id,
                workflow_id=dispatch.workflow_id,
                project=dispatch.project,
                spec=dispatch.spec_folder,
                host=vm_handle.host,
            )

            # 10. Return handle
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

        except BaseException:
            # Clean up on failure — no orphaned VMs (including CancelledError)
            if conn is not None:
                try:
                    await conn.close()
                except Exception:
                    logger.warning("SSH close failed during provision cleanup", exc_info=True)
            try:
                await self._vm_provisioner.release(vm_handle)
            except Exception:
                logger.warning(
                    "Provider release failed for VM %s during provision cleanup",
                    vm_handle.vm_id,
                    exc_info=True,
                )
            finally:
                await self._state_store.record_release(vm_handle.vm_id)
            raise

    async def execute(
        self,
        handle: EnvironmentHandle,
        dispatch: Dispatch,
        config: WorkerConfig,
        *,
        dispatch_stem: str = "",  # noqa: ARG002 — required by protocol interface
    ) -> PhaseResult:
        """Run agent on remote VM with retry logic.

        Returns:
            PhaseResult with outcome, signal, and metrics.

        Raises:
            RuntimeError: If the handle does not contain a remote runtime.
        """
        if handle.runtime.kind != "remote":
            raise RuntimeError("SSHExecutionEnvironment requires remote runtime handle")
        remote_runtime = cast("RemoteEnvironmentRuntime", handle.runtime)
        conn = cast("SSHConnection", remote_runtime.connection)
        workspace = remote_runtime.workspace_path

        start = time.monotonic()
        dispatch_start_utc = datetime.now(UTC)
        transient_retries = 0

        # Build full prompt (skip for bash/gate — no agent prompt needed)
        command_name = dispatch.phase.value
        if dispatch.cli == Cli.BASH:
            prompt_content = ""
        else:
            remote_cmd_path = f"{workspace.path}/{config.commands_dir}/{command_name}.md"
            prompt_content = await conn.download_content(remote_cmd_path) or ""
            if prompt_content:
                prompt_content = f"{prompt_content}\n\n---\n\nSpec folder: {dispatch.spec_folder}\n"
                if dispatch.context:
                    prompt_content += f"\nAdditional context: {dispatch.context}\n"

        while True:
            # Build CLI command
            cli_command = build_cli_command(dispatch, config)
            signal_path = f"{workspace.path}/{dispatch.spec_folder}/.agent-status"

            # Run agent
            agent_result = await self._runner.run(
                conn,
                workspace,
                prompt_content=prompt_content,
                cli_command=cli_command,
                signal_path=signal_path,
                timeout_secs=dispatch.timeout,
            )

            # Extract signal: .agent-status file first, stdout fallback
            signal_token = extract_signal_token(
                command_name,
                agent_result.signal_content or "",
                agent_result.stdout or "",
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
                        "Transient error (attempt %d/3), retrying in %ds. "
                        "exit_code=%d stderr=%.500s stdout=%.500s",
                        transient_retries,
                        backoff,
                        agent_result.exit_code,
                        (agent_result.stderr or "")[-500:],
                        (agent_result.stdout or "")[-500:],
                    )
                    await asyncio.sleep(backoff)
                    continue
                if error_class == ErrorClass.AMBIGUOUS and transient_retries < 1:
                    transient_retries += 1

                    # Signal recovery nudge: agent likely finished but
                    # forgot to write the status file.  Re-invoke with a
                    # short prompt asking it to emit the signal instead of
                    # blindly re-running the entire task.
                    if agent_result.exit_code == 0 and dispatch.cli != Cli.BASH:
                        logger.warning(
                            "No signal detected (exit 0), nudging agent to write status file"
                        )
                        nudge_prompt = (
                            "You completed your task but did not write "
                            "the status file.\n\n"
                            "Write your exit signal to the status file "
                            "AND print it to stdout:\n\n"
                            f'    echo "{command_name}-status: complete"'
                            f" > {dispatch.spec_folder}/.agent-status\n\n"
                            f"Then print: {command_name}-status: complete\n\n"
                            "If you did not complete successfully, use the "
                            "appropriate signal (blocked, error, all-done, "
                            "fail) instead."
                        )
                        nudge_result = await self._runner.run(
                            conn,
                            workspace,
                            prompt_content=nudge_prompt,
                            cli_command=cli_command,
                            signal_path=signal_path,
                            timeout_secs=120,
                        )
                        nudge_token = extract_signal_token(
                            command_name,
                            nudge_result.signal_content or "",
                            nudge_result.stdout or "",
                        )
                        if nudge_token is not None:
                            outcome, signal_val = map_outcome(
                                dispatch.phase,
                                nudge_token,
                                nudge_result.exit_code,
                                nudge_result.timed_out,
                            )
                            agent_result = nudge_result
                            break

                    logger.warning(
                        "Ambiguous error, retrying once in 10s. "
                        "exit_code=%d stderr=%.500s stdout=%.500s",
                        agent_result.exit_code,
                        (agent_result.stderr or "")[-500:],
                        (agent_result.stdout or "")[-500:],
                    )
                    await asyncio.sleep(10)
                    continue

            break

        duration = int(time.monotonic() - start)

        # Remote postflight: push on push phases
        final_stdout = agent_result.stdout
        remote_postflight = None
        if dispatch.phase in PUSH_PHASES and outcome not in (Outcome.ERROR, Outcome.TIMEOUT):
            push_cmd = self._workspace_mgr.push_command(workspace.path, dispatch.branch)
            push_result = await conn.run(
                wrap_for_agent_user(push_cmd, self._agent_user), timeout_secs=120
            )
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
                remote_postflight = PostflightResult(pushed=False, push_error=push_result.stderr)
            else:
                remote_postflight = PostflightResult(pushed=True)

        # Collect token usage (best-effort, 30s timeout)
        token_usage_data = None
        if dispatch.cli != Cli.BASH:
            dispatch_end_utc = datetime.now(UTC)
            usage_runner = RemoteCommandRunner(conn, run_as_user=self._agent_user)
            usage = await collect_token_usage(
                dispatch.cli,
                workspace.path,
                dispatch_start_utc,
                dispatch_end_utc,
                config,
                usage_runner,
            )
            if usage is not None:
                token_usage_data = usage

        # Capture gate output for gate phases so it's visible in results
        captured_gate_output = None
        if dispatch.phase == Phase.GATE:
            combined = (final_stdout or "") + (agent_result.stderr or "")
            if combined.strip():
                captured_gate_output = combined

        return PhaseResult(
            outcome=outcome,
            signal=signal_val,
            exit_code=agent_result.exit_code,
            stdout=final_stdout,
            stderr=agent_result.stderr,
            duration_secs=duration,
            preflight_passed=True,
            postflight_result=remote_postflight,
            env_report=None,
            gate_output=captured_gate_output,
            unchecked_tasks=0,
            plan_hash="00000000",
            retries=transient_retries,
            token_usage=token_usage_data,
        )

    async def get_access_info(self, handle: EnvironmentHandle) -> AccessInfo:
        """Return SSH and VS Code connection info.

        Raises:
            RuntimeError: If the handle does not contain a remote runtime.
        """
        if handle.runtime.kind != "remote":
            raise RuntimeError("SSHExecutionEnvironment requires remote runtime handle")
        remote_runtime = cast("RemoteEnvironmentRuntime", handle.runtime)
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
        remote_runtime = cast("RemoteEnvironmentRuntime", handle.runtime)
        conn = cast("SSHConnection", remote_runtime.connection)
        workspace = remote_runtime.workspace_path
        teardown_cmds = remote_runtime.teardown_commands
        vm_handle = remote_runtime.vm_handle

        try:
            for cmd in teardown_cmds:
                try:
                    teardown_cmd = f"cd {shlex.quote(workspace.path)} && {cmd}"
                    await conn.run(
                        wrap_for_agent_user(teardown_cmd, self._agent_user),
                        timeout_secs=120,
                    )
                except Exception:
                    logger.warning("Teardown command failed: %s", cmd, exc_info=True)
        finally:
            try:
                await self._workspace_mgr.cleanup(conn, workspace)
                # Remove credential files (best-effort, after workspace cleanup)
                raw_paths = all_credential_cleanup_paths(self._credential_providers)
                home = f"/home/{self._agent_user}" if self._agent_user else "/root"
                cred_paths = [p.replace("~", home) for p in raw_paths]
                for cred_path in cred_paths:
                    try:
                        await conn.run(f"rm -f {shlex.quote(cred_path)}", timeout_secs=10)
                    except Exception:
                        logger.warning("Credential cleanup failed: %s", cred_path, exc_info=True)
            except Exception:
                logger.warning("Workspace cleanup failed", exc_info=True)
            finally:
                try:
                    await conn.close()
                except Exception:
                    logger.warning("SSH close failed", exc_info=True)
                finally:
                    # MUST happen — no orphaned VMs
                    try:
                        await self._vm_provisioner.release(vm_handle)
                    except Exception:
                        logger.warning(
                            "Provider release failed for VM %s during teardown",
                            vm_handle.vm_id,
                            exc_info=True,
                        )
                    await self._state_store.record_release(vm_handle.vm_id)

    async def _await_ssh_ready(
        self,
        conn: SSHConnection,
        *,
        timeout_secs: int = _SSH_READY_TIMEOUT_SECS,
    ) -> None:
        """Poll SSH until the host accepts connections or deadline expires.

        Raises:
            TimeoutError: If SSH is not reachable within the timeout.
        """
        deadline = time.monotonic() + timeout_secs
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
        raise TimeoutError(
            f"SSH not reachable within {timeout_secs}s on {conn.get_host_identifier()}"
        )
