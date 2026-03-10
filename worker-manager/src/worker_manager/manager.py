"""Top-level worker manager: poll loop, dispatch handling, startup/shutdown."""

import asyncio
import contextlib
import logging
import os
import signal
from datetime import UTC, datetime
from pathlib import Path

from worker_manager.adapters import (
    DispatchReceived,
    DotenvEnvProvisioner,
    DotenvEnvValidator,
    ErrorOccurred,
    GitPostflightRunner,
    GitPreflightRunner,
    GitWorktreeManager,
    NullEventEmitter,
    PhaseCompleted,
    PostflightCompleted,
    PreflightCompleted,
    SqliteEventEmitter,
    SubprocessSpawner,
)
from worker_manager.adapters.local_environment import LocalExecutionEnvironment
from worker_manager.adapters.protocols import (
    EnvProvisioner,
    EnvValidator,
    EventEmitter,
    ExecutionEnvironment,
    PostflightRunner,
    PreflightRunner,
    ProcessSpawner,
    WorktreeManager,
)
from worker_manager.adapters.types import ProvisionError
from worker_manager.config import Config
from worker_manager.heartbeat import HeartbeatWriter
from worker_manager.ipc import (
    atomic_write,
    delete_file,
    scan_dispatch_dir,
    write_nudge,
    write_result,
)
from worker_manager.queues import DispatchRouter
from worker_manager.remote_config import load_remote_config
from worker_manager.schemas import (
    Dispatch,
    FindingSeverity,
    Nudge,
    Outcome,
    Phase,
    Result,
    WorkerHealth,
    parse_issue_from_workflow_id,
)
from worker_manager.signals import (
    parse_audit_findings,
    parse_audit_spec_findings,
    parse_demo_findings,
    parse_investigation_report,
)

logger = logging.getLogger(__name__)

_PUSH_PHASES = frozenset({Phase.DO_TASK, Phase.AUDIT_TASK, Phase.RUN_DEMO, Phase.AUDIT_SPEC})

_GATE_OUTPUT_LINES_SUCCESS = 100
_GATE_OUTPUT_LINES_FAIL = 300
_TAIL_OUTPUT_LINES = 200


def _build_gate_output(stdout: str | None, outcome: Outcome) -> str | None:
    """Extract last N lines of stdout for gate phases.

    Returns more lines on failure (300) than success (100) so the coordinator
    sees enough diagnostic detail to act on gate failures.
    """
    if not stdout:
        return None
    limit = _GATE_OUTPUT_LINES_SUCCESS if outcome == Outcome.SUCCESS else _GATE_OUTPUT_LINES_FAIL
    lines = stdout.strip().split("\n")
    return "\n".join(lines[-limit:])


def _build_tail_output(stdout: str | None) -> str | None:
    """Extract last 200 lines of stdout for operational visibility."""
    if not stdout:
        return None
    lines = stdout.strip().split("\n")
    return "\n".join(lines[-_TAIL_OUTPUT_LINES:])


class WorkerManager:
    """Host-level service that polls for dispatches, spawns agents, and writes results.

    Lifecycle: poll dispatch/ → route to queue → provision environment →
    execute agent process → extract signal → build result → write to results/.
    Setup/cleanup phases manage worktree lifecycle. Work phases delegate to
    an ExecutionEnvironment (local subprocess by default, Docker/VM in future).
    """

    def __init__(
        self,
        config: Config | None = None,
        *,
        execution_env: ExecutionEnvironment | None = None,
        worktree_mgr: WorktreeManager | None = None,
        preflight: PreflightRunner | None = None,
        postflight: PostflightRunner | None = None,
        spawner: ProcessSpawner | None = None,
        env_validator: EnvValidator | None = None,
        env_provisioner: EnvProvisioner | None = None,
        emitter: EventEmitter | None = None,
    ) -> None:
        self._config = config or Config.from_env()
        self._shutdown_event = asyncio.Event()
        self._dispatch_dir = Path(self._config.ipc_dir) / "dispatch"
        self._results_dir = Path(self._config.ipc_dir) / "results"
        self._in_progress_dir = Path(self._config.ipc_dir) / "in-progress"
        self._input_dir = Path(self._config.ipc_dir) / "input"
        self._heartbeat = HeartbeatWriter(self._in_progress_dir, self._config.heartbeat_interval)
        self._router = DispatchRouter(
            handler=self._handle_dispatch,
            max_impl=self._config.max_opencode,
            max_audit=self._config.max_codex,
            max_gate=self._config.max_gate,
        )

        # Adapters — default to concrete implementations when not injected
        self._worktree_mgr = worktree_mgr or GitWorktreeManager()
        self._preflight = preflight or GitPreflightRunner()
        self._postflight = postflight or GitPostflightRunner()
        self._spawner = spawner or SubprocessSpawner()
        self._env_validator = env_validator or DotenvEnvValidator()
        self._env_provisioner = env_provisioner or DotenvEnvProvisioner()

        # Build execution environment — use injected or construct from config
        if execution_env is not None:
            self._execution_env = execution_env
        elif self._config.remote_config_path:
            self._execution_env = self._build_remote_env()
        else:
            self._execution_env = LocalExecutionEnvironment(
                env_validator=self._env_validator,
                preflight=self._preflight,
                postflight=self._postflight,
                spawner=self._spawner,
                heartbeat=self._heartbeat,
                config=self._config,
            )

        # Event emitter — auto-configure from config if not injected
        if emitter is not None:
            self._emitter: EventEmitter = emitter
        elif self._config.events_db:
            self._emitter = SqliteEventEmitter(self._config.events_db)
        else:
            self._emitter = NullEventEmitter()

    async def run(self) -> None:
        """Main entry point: setup, poll loop, shutdown."""
        logging.basicConfig(
            level=logging.INFO,
            format="%(asctime)s %(levelname)s %(name)s: %(message)s",
        )

        logger.info("Worker manager starting")
        logger.info("IPC dir: %s", self._config.ipc_dir)
        logger.info("Data dir: %s", self._config.data_dir)

        # Register signal handlers
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGTERM, signal.SIGINT):
            loop.add_signal_handler(sig, self._signal_shutdown)

        # Ensure IPC subdirs exist
        for d in (self._dispatch_dir, self._results_dir, self._in_progress_dir):
            d.mkdir(parents=True, exist_ok=True)

        # Ensure data dir exists
        Path(self._config.data_dir).mkdir(parents=True, exist_ok=True)

        # Startup recovery: clean stale heartbeats
        await self._heartbeat.cleanup_stale()

        # Startup recovery: check active VM assignments
        await self._recover_vm_state()

        self._started_at = datetime.now(UTC).isoformat()

        # Start consumer tasks
        self._router.start_consumers()

        # Start poll loop
        poll_task = asyncio.create_task(self._poll_loop(), name="poll-loop")

        logger.info("Worker manager ready — polling every %.1fs", self._config.poll_interval)

        # Await shutdown
        await self._shutdown_event.wait()

        logger.info("Shutting down...")

        # Cancel poll loop
        poll_task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await poll_task

        # Stop consumers
        await self._router.stop()

        # Close event emitter
        await self._emitter.close()

        logger.info("Worker manager stopped")

    def _signal_shutdown(self) -> None:
        """Handle SIGTERM/SIGINT."""
        logger.info("Received shutdown signal")
        self._shutdown_event.set()

    async def _poll_loop(self) -> None:
        """Poll dispatch directory every poll_interval seconds."""
        try:
            while not self._shutdown_event.is_set():
                try:
                    dispatches = await scan_dispatch_dir(self._dispatch_dir)
                    for path, dispatch in dispatches:
                        logger.info(
                            "Picked up dispatch: %s phase=%s cli=%s",
                            dispatch.workflow_id,
                            dispatch.phase,
                            dispatch.cli,
                        )
                        # Delete dispatch file immediately (read-once semantics)
                        await delete_file(path)
                        # Route to appropriate queue
                        self._router.route(path, dispatch)
                except Exception:
                    logger.exception("Error in poll loop")

                # Write health file at end of each poll cycle
                await self._write_health()

                await asyncio.sleep(self._config.poll_interval)
        except asyncio.CancelledError:
            pass

    async def _write_health(self) -> None:
        """Write worker-health.json to IPC dir."""
        active, queued = self._router.get_stats()
        health = WorkerHealth(
            pid=os.getpid(),
            started_at=self._started_at,
            last_poll=datetime.now(UTC).isoformat(),
            active_processes=active,
            queued_dispatches=queued,
        )
        health_path = Path(self._config.ipc_dir) / "worker-health.json"
        await atomic_write(health_path, health.model_dump_json(indent=2))

    def _build_remote_env(self) -> ExecutionEnvironment:
        """Construct SSHExecutionEnvironment from remote.yml."""
        import os

        from worker_manager.adapters.git_workspace import (
            GitAuthConfig,
            GitWorkspaceManager,
        )
        from worker_manager.adapters.manual_vm import ManualVMProvisioner
        from worker_manager.adapters.remote_runner import RemoteAgentRunner
        from worker_manager.adapters.sqlite_vm_state import SqliteVMStateStore
        from worker_manager.adapters.ssh import SSHConfig
        from worker_manager.adapters.ssh_environment import (
            SSHExecutionEnvironment,
        )
        from worker_manager.adapters.ubuntu_bootstrap import (
            UbuntuBootstrapper,
        )
        from worker_manager.secrets import SecretConfig, SecretLoader

        assert self._config.remote_config_path is not None
        remote_cfg = load_remote_config(self._config.remote_config_path)

        ssh_defaults = SSHConfig(
            host="",  # placeholder — overridden per VM
            user=remote_cfg.ssh.user,
            key_path=remote_cfg.ssh.key_path,
            connect_timeout=remote_cfg.ssh.connect_timeout,
        )

        token = os.environ.get(remote_cfg.git.token_env, "")
        git_auth = GitAuthConfig(
            auth_method=remote_cfg.git.auth,
            token=token or None,
        )

        state_store = SqliteVMStateStore(
            f"{self._config.data_dir}/vm-state.db"
        )

        # Read extra bootstrap script if configured
        extra_script = None
        if remote_cfg.bootstrap.extra_script:
            script_path = Path(remote_cfg.bootstrap.extra_script).expanduser()
            if script_path.exists():
                extra_script = script_path.read_text()
            else:
                logger.warning(
                    "Bootstrap extra script not found: %s", script_path
                )

        secret_config = SecretConfig(
            developer_secrets_path=(
                remote_cfg.secrets.developer_secrets_path
                or SecretConfig().developer_secrets_path
            ),
        )

        return SSHExecutionEnvironment(
            vm_provisioner=ManualVMProvisioner(
                remote_cfg.vms, state_store
            ),
            bootstrapper=UbuntuBootstrapper(extra_script=extra_script),
            workspace_mgr=GitWorkspaceManager(git_auth),
            runner=RemoteAgentRunner(),
            state_store=state_store,
            secret_loader=SecretLoader(secret_config),
            emitter=self._emitter,
            ssh_config_defaults=ssh_defaults,
            repo_urls=remote_cfg.repos,
            bootstrap_extra_script=extra_script,
        )

    async def _recover_vm_state(self) -> None:
        """Startup recovery: check active assignments, release unreachable VMs."""
        if not self._config.remote_config_path:
            return

        from worker_manager.adapters.sqlite_vm_state import SqliteVMStateStore
        from worker_manager.adapters.ssh import SSHConfig, SSHConnection

        store = SqliteVMStateStore(f"{self._config.data_dir}/vm-state.db")
        try:
            assignments = await store.get_active_assignments()
            if not assignments:
                return

            logger.info(
                "Checking %d active VM assignment(s) for recovery...",
                len(assignments),
            )
            for a in assignments:
                conn = SSHConnection(SSHConfig(host=a.host))
                try:
                    reachable = await conn.check_connection()
                    if not reachable:
                        await store.record_release(a.vm_id)
                        logger.warning(
                            "Released unreachable VM %s (%s)",
                            a.vm_id,
                            a.host,
                        )
                except Exception:
                    await store.record_release(a.vm_id)
                    logger.warning(
                        "Released VM %s (%s) due to connection error",
                        a.vm_id,
                        a.host,
                    )
                finally:
                    await conn.close()
        finally:
            await store.close()

    async def _handle_dispatch(self, path: Path, dispatch: Dispatch) -> None:
        """Handle a single dispatch through its full lifecycle."""
        dispatch_stem = path.stem
        issue = parse_issue_from_workflow_id(dispatch.workflow_id)
        worktree_path = Path(self._config.github_dir) / f"{dispatch.project}-wt-{issue}"

        now = datetime.now(UTC).isoformat()

        # Emit DispatchReceived
        await self._emitter.emit(
            DispatchReceived(
                timestamp=now,
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase.value,
                project=dispatch.project,
                cli=dispatch.cli.value,
            )
        )

        try:
            # Setup phase: create worktree
            if dispatch.phase == Phase.SETUP:
                await self._handle_setup(dispatch, issue, worktree_path)
                return

            # Cleanup phase: remove worktree
            if dispatch.phase == Phase.CLEANUP:
                await self._handle_cleanup(dispatch)
                return

            # All other phases: validate worktree, spawn process, extract results
            await self._handle_work_phase(path, dispatch, dispatch_stem, worktree_path)

        except Exception as exc:
            logger.exception("Unhandled error in dispatch %s", dispatch.workflow_id)

            await self._emitter.emit(
                ErrorOccurred(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    phase=dispatch.phase.value,
                    error=str(exc),
                    error_class=None,
                )
            )

            # Write error result
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=-1,
                duration_secs=0,
                gate_output=None,
                tail_output="Worker manager internal error",
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )
            await self._write_result_and_nudge(result, dispatch.workflow_id)

    async def _handle_setup(self, dispatch: Dispatch, issue: int, worktree_path: Path) -> None:
        """Handle setup phase: create worktree + register."""
        import time

        start = time.monotonic()
        try:
            wt_path = await self._worktree_mgr.create(
                dispatch.project, issue, dispatch.branch, self._config.github_dir
            )
            project_dir = Path(self._config.github_dir) / dispatch.project
            count = await asyncio.to_thread(self._env_provisioner.provision, wt_path, project_dir)
            if count:
                logger.info("Provisioned %d env vars in worktree .env", count)
            await self._worktree_mgr.register(
                Path(self._config.worktree_registry_path),
                dispatch.workflow_id,
                dispatch.project,
                issue,
                dispatch.branch,
                wt_path,
                self._config.github_dir,
            )
            duration = int(time.monotonic() - start)
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=Phase.SETUP,
                outcome=Outcome.SUCCESS,
                signal=None,
                exit_code=0,
                duration_secs=duration,
                gate_output=None,
                tail_output=None,
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )
        except Exception as e:
            duration = int(time.monotonic() - start)
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=Phase.SETUP,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=1,
                duration_secs=duration,
                gate_output=None,
                tail_output=str(e),
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )

        await self._write_result_and_nudge(result, dispatch.workflow_id)

    async def _handle_cleanup(self, dispatch: Dispatch) -> None:
        """Handle cleanup phase: remove worktree + registry entry."""
        import time

        start = time.monotonic()
        try:
            await self._worktree_mgr.cleanup(
                dispatch.workflow_id,
                Path(self._config.worktree_registry_path),
                self._config.github_dir,
            )
            duration = int(time.monotonic() - start)
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=Phase.CLEANUP,
                outcome=Outcome.SUCCESS,
                signal=None,
                exit_code=0,
                duration_secs=duration,
                gate_output=None,
                tail_output=None,
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )
        except Exception as e:
            duration = int(time.monotonic() - start)
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=Phase.CLEANUP,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=1,
                duration_secs=duration,
                gate_output=None,
                tail_output=str(e),
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )

        await self._write_result_and_nudge(result, dispatch.workflow_id)

    async def _handle_work_phase(
        self,
        path: Path,
        dispatch: Dispatch,
        dispatch_stem: str,
        worktree_path: Path,
    ) -> None:
        """Handle agent/gate phases: provision, execute, report."""
        import time

        start = time.monotonic()

        # 1. Provision
        try:
            handle = await self._execution_env.provision(dispatch, self._config)

            await self._emitter.emit(
                PreflightCompleted(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    passed=True,
                    repairs=handle._preflight_result.repairs if handle._preflight_result else [],
                )
            )

        except ProvisionError as e:
            # Emit PreflightCompleted(passed=False) if preflight ran
            await self._emitter.emit(
                PreflightCompleted(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    passed=False,
                    repairs=[],
                )
            )
            await self._write_result_and_nudge(e.result, dispatch.workflow_id)
            return

        # 2. Execute
        try:
            phase_result = await self._execution_env.execute(
                handle, dispatch, self._config, dispatch_stem=dispatch_stem
            )

            duration = phase_result.duration_secs
            spec_folder_path = worktree_path / dispatch.spec_folder

            # 3. Build gate_output (gate phases only)
            gate_output = None
            if dispatch.phase == Phase.GATE:
                gate_output = _build_gate_output(phase_result.stdout, phase_result.outcome)

            # 4. Build tail_output
            tail_output = None
            is_agent_phase = dispatch.phase not in (
                Phase.GATE,
                Phase.SETUP,
                Phase.CLEANUP,
            )
            if is_agent_phase or phase_result.outcome != Outcome.SUCCESS:
                tail_output = _build_tail_output(phase_result.stdout)

            # 5. Apply postflight results
            pushed: bool | None = None
            integrity_repairs: dict | None = None
            spec_modified = False
            if phase_result.postflight_result is not None:
                postflight_result = phase_result.postflight_result

                await self._emitter.emit(
                    PostflightCompleted(
                        timestamp=datetime.now(UTC).isoformat(),
                        workflow_id=dispatch.workflow_id,
                        phase=dispatch.phase.value,
                        pushed=postflight_result.pushed,
                        integrity_repairs=postflight_result.integrity_repairs,
                    )
                )

                pushed = postflight_result.pushed
                integrity_repairs = postflight_result.integrity_repairs
                spec_modified = postflight_result.integrity_repairs.get("spec_reverted", False)
                if not postflight_result.pushed and postflight_result.push_error:
                    if tail_output:
                        tail_output += (
                            f"\n\n--- git push failed ---\n{postflight_result.push_error}"
                        )
                    else:
                        tail_output = f"git push failed: {postflight_result.push_error}"

            # 6. Parse structured findings
            new_tasks, findings_data = self._parse_findings(dispatch, spec_folder_path)

            # 7. Construct Result
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=phase_result.outcome,
                signal=phase_result.signal,
                exit_code=phase_result.exit_code,
                duration_secs=duration,
                gate_output=gate_output,
                tail_output=tail_output,
                unchecked_tasks=phase_result.unchecked_tasks,
                plan_hash=phase_result.plan_hash,
                spec_modified=spec_modified,
                pushed=pushed,
                integrity_repairs=integrity_repairs,
                new_tasks=new_tasks,
                findings=findings_data,
            )

            # 8. Emit PhaseCompleted
            await self._emitter.emit(
                PhaseCompleted(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    phase=dispatch.phase.value,
                    outcome=phase_result.outcome.value,
                    signal=phase_result.signal,
                    duration_secs=duration,
                    exit_code=phase_result.exit_code,
                )
            )

        except Exception as e:
            duration = int(time.monotonic() - start)
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=-1,
                duration_secs=duration,
                gate_output=None,
                tail_output=str(e),
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )

        finally:
            await self._execution_env.teardown(handle)

        await self._write_result_and_nudge(result, dispatch.workflow_id)

    def _parse_findings(
        self, dispatch: Dispatch, spec_folder_path: Path
    ) -> tuple[list[dict], list[dict]]:
        """Parse structured findings from audit/demo/investigate phases."""
        new_tasks: list[dict] = []
        findings_data: list[dict] = []

        if dispatch.phase == Phase.AUDIT_TASK:
            findings = parse_audit_findings(spec_folder_path)
            if findings:
                findings_data = [f.model_dump() for f in findings.findings]
                new_tasks = [
                    f.model_dump() for f in findings.findings if f.severity == FindingSeverity.FIX
                ]
        elif dispatch.phase == Phase.RUN_DEMO:
            findings = parse_demo_findings(spec_folder_path)
            if findings:
                findings_data = [f.model_dump() for f in findings.findings]
                new_tasks = [
                    f.model_dump() for f in findings.findings if f.severity == FindingSeverity.FIX
                ]
        elif dispatch.phase == Phase.AUDIT_SPEC:
            spec_findings = parse_audit_spec_findings(spec_folder_path)
            if spec_findings:
                findings_data = [f.model_dump() for f in spec_findings]
                new_tasks = [
                    f.model_dump() for f in spec_findings if f.severity == FindingSeverity.FIX
                ]
        elif dispatch.phase == Phase.INVESTIGATE:
            report = parse_investigation_report(spec_folder_path)
            if report:
                findings_data = [{"report": report.model_dump()}]
                for rc in report.root_causes:
                    new_tasks.extend(rc.suggested_tasks)

        return new_tasks, findings_data

    async def _write_result_and_nudge(self, result: Result, workflow_id: str) -> None:
        """Write result to results/ and nudge to input/."""
        await write_result(self._results_dir, result)
        logger.info(
            "Result written: %s phase=%s outcome=%s signal=%s",
            result.workflow_id,
            result.phase,
            result.outcome,
            result.signal,
        )

        nudge = Nudge(workflow_id=workflow_id)
        await write_nudge(self._input_dir, nudge)
        logger.info("Nudge written for %s", workflow_id)
