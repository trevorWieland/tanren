"""Top-level worker manager: poll loop, dispatch handling, startup/shutdown.

Reference docs:
- docs/workflow/spec-lifecycle.md
- protocol/PROTOCOL.md
"""

import asyncio
import contextlib
import logging
import os
import signal
import subprocess  # noqa: S404 — subprocess used for local process management
import time
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING, cast

if TYPE_CHECKING:
    import asyncpg
    from pydantic import JsonValue

    from tanren_core.adapters.protocols import (
        EnvProvisioner,
        EnvValidator,
        EventEmitter,
        ExecutionEnvironment,
        PostflightRunner,
        PreflightRunner,
        ProcessSpawner,
        VMStateStore,
        WorktreeManager,
    )
    from tanren_core.adapters.protocols import VMProvisioner as VMProvisionerProtocol
    from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment


from tanren_core.adapters import (
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
    TokenUsageRecorded,
)
from tanren_core.adapters.local_environment import LocalExecutionEnvironment
from tanren_core.adapters.manual_vm import ManualProvisionerSettings, ManualVMProvisioner
from tanren_core.adapters.postgres_pool import is_postgres_url
from tanren_core.adapters.remote_types import VMHandle, VMProvider, WorkspacePath
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore
from tanren_core.adapters.types import (
    EnvironmentHandle,
    LocalEnvironmentRuntime,
    PhaseResult,
    ProvisionError,
    RemoteEnvironmentRuntime,
)
from tanren_core.ccusage import (
    LocalCommandRunner,
    collect_token_usage,
)
from tanren_core.config import Config
from tanren_core.heartbeat import HeartbeatWriter
from tanren_core.ipc import (
    atomic_write,
    delete_checkpoint,
    delete_file,
    list_checkpoints,
    read_checkpoint,
    scan_dispatch_dir,
    write_checkpoint,
    write_nudge,
    write_result,
)
from tanren_core.queues import DispatchRouter
from tanren_core.remote_config import ProvisionerType, load_remote_config
from tanren_core.schemas import (
    Checkpoint,
    CheckpointStage,
    Cli,
    Dispatch,
    FindingSeverity,
    Nudge,
    Outcome,
    Phase,
    Result,
    WorkerHealth,
    parse_issue_from_workflow_id,
)
from tanren_core.signals import (
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

    More lines on failure (300) than success (100) so the coordinator
    sees enough diagnostic detail to act on gate failures.

    Returns:
        Truncated stdout string, or None if stdout is empty.
    """
    if not stdout:
        return None
    limit = _GATE_OUTPUT_LINES_SUCCESS if outcome == Outcome.SUCCESS else _GATE_OUTPUT_LINES_FAIL
    lines = stdout.strip().split("\n")
    return "\n".join(lines[-limit:])


def build_tail_output(stdout: str | None) -> str | None:
    """Extract last 200 lines of stdout for operational visibility.

    Returns:
        Truncated stdout string, or None if stdout is empty.
    """
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
        vm_state_store: VMStateStore | None = None,
    ) -> None:
        """Initialize with optional config and adapter overrides."""
        self._config = config or Config.from_env()
        self._shutdown_event = asyncio.Event()
        self._dispatch_dir = Path(self._config.ipc_dir) / "dispatch"
        self._results_dir = Path(self._config.ipc_dir) / "results"
        self._in_progress_dir = Path(self._config.ipc_dir) / "in-progress"
        self._input_dir = Path(self._config.ipc_dir) / "input"
        self._checkpoints_dir = Path(self._config.checkpoints_dir)
        self._checkpoints_dir.mkdir(parents=True, exist_ok=True)
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

        # Postgres state (populated in run() if events_db is a Postgres URL)
        self._pg_dsn: str | None = None
        self._pg_pool: asyncpg.Pool | None = None

        # Event emitter — auto-configure from config if not injected
        # Must be initialized before execution environment (remote env references it)
        if emitter is not None:
            self._emitter: EventEmitter = emitter
        elif self._config.events_db and is_postgres_url(self._config.events_db):
            # Defer Postgres pool creation to run() (needs async context)
            self._pg_dsn = self._config.events_db
            self._emitter = NullEventEmitter()
        elif self._config.events_db:
            self._emitter = SqliteEventEmitter(self._config.events_db)
        else:
            self._emitter = NullEventEmitter()

        # Build execution environment — use injected or construct from config
        self._execution_env_injected = execution_env is not None
        if execution_env is not None:
            self._execution_env = execution_env
            # When execution_env is injected but remote config exists,
            # wire the VM state store so resume can verify VM assignments.
            # Prefer the caller-supplied store (which matches the provisioning backend);
            # fall back to Sqlite if none was provided.
            if self._config.remote_config_path:
                self._remote_state_store = vm_state_store or SqliteVMStateStore(
                    f"{self._config.data_dir}/vm-state.db"
                )
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

    def get_execution_environment(self) -> ExecutionEnvironment:
        """Return the configured execution environment."""
        return self._execution_env

    async def run(self) -> None:
        """Main entry point: setup, poll loop, shutdown."""
        logging.basicConfig(
            level=logging.INFO,
            format="%(asctime)s %(levelname)s %(name)s: %(message)s",
        )

        logger.info("Worker manager starting")
        logger.info("IPC dir: %s", self._config.ipc_dir)
        logger.info("Data dir: %s", self._config.data_dir)

        # Initialize Postgres pool if configured
        await self._init_postgres()

        # Register signal handlers
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGTERM, signal.SIGINT):
            loop.add_signal_handler(sig, self._signal_shutdown)

        # Ensure IPC subdirs exist
        for d in (self._dispatch_dir, self._results_dir, self._in_progress_dir):
            d.mkdir(parents=True, exist_ok=True)

        # Ensure data dir exists
        Path(self._config.data_dir).mkdir(parents=True, exist_ok=True)  # noqa: ASYNC240 — startup-time dir creation

        # Startup recovery: clean stale heartbeats
        await self._heartbeat.cleanup_stale()

        # Startup recovery: resume checkpoints BEFORE releasing stale VMs,
        # because checkpoints may reference VMs that are still active.
        await self._recover_checkpoints()

        # Collect VM IDs still owned by retained checkpoints (failed resumes)
        # so _recover_vm_state skips them.
        retained_vm_ids = await self._get_checkpoint_vm_ids()

        # Startup recovery: release any remaining stale VM assignments
        # not covered by checkpoints.
        await self._recover_vm_state(skip_vm_ids=retained_vm_ids)

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

        # Close remote state store if present
        if hasattr(self, "_remote_state_store"):
            await self._remote_state_store.close()

        # Close Postgres pool if present
        if self._pg_pool is not None:
            await self._pg_pool.close()

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

    def _build_remote_env(self, pool: asyncpg.Pool | None = None) -> ExecutionEnvironment:
        """Construct SSHExecutionEnvironment from remote.yml.

        Args:
            pool: Optional asyncpg.Pool for Postgres-backed state store.

        Returns:
            Configured SSHExecutionEnvironment.
        """
        from tanren_core.builder import (  # noqa: PLC0415 — avoid circular import
            build_ssh_execution_environment,
        )

        env, state_store = build_ssh_execution_environment(self._config, self._emitter, pool=pool)
        self._remote_state_store = state_store
        return env

    async def _init_postgres(self) -> None:
        """Create Postgres pool and rebuild adapters if configured."""
        if self._pg_dsn is None:
            return

        from tanren_core.adapters.postgres_emitter import (  # noqa: PLC0415 — conditional import based on configuration
            PostgresEventEmitter,
        )
        from tanren_core.adapters.postgres_pool import (  # noqa: PLC0415 — conditional import based on configuration
            create_postgres_pool,
        )

        pool = await create_postgres_pool(self._pg_dsn)
        self._pg_pool = pool
        self._emitter = PostgresEventEmitter(pool)

        # Rebuild remote execution environment with Postgres-backed state store,
        # but only if the execution environment was not explicitly injected.
        if self._config.remote_config_path and not self._execution_env_injected:
            if hasattr(self, "_remote_state_store"):
                await self._remote_state_store.close()
            self._execution_env = self._build_remote_env(pool=pool)

    async def _recover_vm_state(self, *, skip_vm_ids: frozenset[str] = frozenset()) -> None:
        """Startup recovery: release stale assignments via provider cleanup.

        Args:
            skip_vm_ids: VM IDs to skip (owned by retained checkpoints).

        Raises:
            ValueError: If the provisioner type in remote.yml is unsupported.
        """
        if not self._config.remote_config_path:
            return

        remote_cfg = load_remote_config(self._config.remote_config_path)

        if self._pg_pool is not None:
            from tanren_core.adapters.postgres_vm_state import (  # noqa: PLC0415 — conditional import based on configuration
                PostgresVMStateStore,
            )

            store = PostgresVMStateStore(self._pg_pool)
        else:
            store = SqliteVMStateStore(f"{self._config.data_dir}/vm-state.db")
        try:
            assignments = await store.get_active_assignments()
            if not assignments:
                return

            logger.info(
                "Recovering %d stale VM assignment(s) at startup...",
                len(assignments),
            )

            vm_provisioner: VMProvisionerProtocol
            provider: VMProvider
            try:
                if remote_cfg.provisioner.type == ProvisionerType.MANUAL:
                    manual_settings = ManualProvisionerSettings.from_settings(
                        remote_cfg.provisioner.settings
                    )
                    vm_provisioner = ManualVMProvisioner(list(manual_settings.vms), store)
                    provider = VMProvider.MANUAL
                elif remote_cfg.provisioner.type == ProvisionerType.HETZNER:
                    from tanren_core.adapters.hetzner_vm import (  # noqa: PLC0415 — deferred import for optional dependency
                        HetznerProvisionerSettings,
                        HetznerVMProvisioner,
                    )

                    hetzner_settings = HetznerProvisionerSettings.from_settings(
                        remote_cfg.provisioner.settings
                    )
                    vm_provisioner = HetznerVMProvisioner(hetzner_settings)
                    provider = VMProvider.HETZNER
                elif remote_cfg.provisioner.type == ProvisionerType.GCP:
                    from tanren_core.adapters.gcp_vm import (  # noqa: PLC0415 — deferred import for optional dependency
                        GCPProvisionerSettings,
                        GCPVMProvisioner,
                    )

                    gcp_settings = GCPProvisionerSettings.from_settings(
                        remote_cfg.provisioner.settings
                    )
                    vm_provisioner = GCPVMProvisioner(gcp_settings)
                    provider = VMProvider.GCP
                else:
                    raise ValueError(
                        f"Unsupported provisioner type for recovery: {remote_cfg.provisioner.type}"
                    )
            except Exception:
                logger.warning(
                    "Skipping stale VM cleanup: unable to initialize VM provisioner",
                    exc_info=True,
                )
                return

            for a in assignments:
                if a.vm_id in skip_vm_ids:
                    logger.info(
                        "Skipping VM %s (%s) — owned by retained checkpoint",
                        a.vm_id,
                        a.host,
                    )
                    continue
                stale_handle = VMHandle(
                    vm_id=a.vm_id,
                    host=a.host,
                    provider=provider,
                    created_at=a.assigned_at,
                )
                try:
                    await vm_provisioner.release(stale_handle)
                except Exception:
                    logger.warning(
                        "Failed provider release during stale VM recovery: %s (%s)",
                        a.vm_id,
                        a.host,
                        exc_info=True,
                    )
                finally:
                    await store.record_release(a.vm_id)
                    logger.warning("Recovered stale VM %s (%s) at startup", a.vm_id, a.host)
        finally:
            await store.close()

    async def _handle_dispatch(self, path: Path, dispatch: Dispatch) -> None:
        """Handle a single dispatch through its full lifecycle."""
        dispatch_stem = path.stem
        issue = parse_issue_from_workflow_id(dispatch.workflow_id, project=dispatch.project)
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

    async def _handle_setup(self, dispatch: Dispatch, issue: str, worktree_path: Path) -> None:  # noqa: ARG002 — required by interface
        """Handle setup phase: create worktree + register."""
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
        path: Path,  # noqa: ARG002 — required by interface
        dispatch: Dispatch,
        dispatch_stem: str,
        worktree_path: Path,
    ) -> None:
        """Handle agent/gate phases: provision, execute, post-process, write result.

        Writes progressive checkpoints at each phase boundary so that
        partially completed dispatches can be resumed after a crash.
        """
        start = time.monotonic()
        now = datetime.now(UTC).isoformat()

        # Write initial checkpoint (DISPATCHED)
        checkpoint = Checkpoint(
            workflow_id=dispatch.workflow_id,
            stage=CheckpointStage.DISPATCHED,
            dispatch_json=dispatch.model_dump_json(),
            worktree_path=str(worktree_path),
            dispatch_stem=dispatch_stem,
            created_at=now,
            updated_at=now,
        )
        await write_checkpoint(self._checkpoints_dir, checkpoint)

        # 1. Provision
        try:
            handle = await self._provision_phase(dispatch)
        except ProvisionError as e:
            await self._emitter.emit(
                PreflightCompleted(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    passed=False,
                    repairs=[],
                )
            )
            await self._write_result_and_nudge(e.result, dispatch.workflow_id)
            await delete_checkpoint(self._checkpoints_dir, dispatch.workflow_id)
            return

        # Update checkpoint to PROVISIONED
        checkpoint.stage = CheckpointStage.PROVISIONED
        checkpoint.vm_id = self._extract_vm_id(handle)
        checkpoint.environment_profile = dispatch.environment_profile
        if handle.runtime.kind == "remote":
            remote_rt = cast("RemoteEnvironmentRuntime", handle.runtime)
            checkpoint.workspace_remote_path = remote_rt.workspace_path.path
        checkpoint.updated_at = datetime.now(UTC).isoformat()
        await write_checkpoint(self._checkpoints_dir, checkpoint)

        # 2. Execute + 3. Post-process
        dispatch_start_utc = datetime.now(UTC)
        failed = False
        try:
            phase_result = await self._execute_phase(handle, dispatch, dispatch_stem)

            # Update checkpoint to EXECUTED
            checkpoint.stage = CheckpointStage.EXECUTED
            checkpoint.phase_result_json = phase_result.model_dump_json()
            checkpoint.dispatch_start_utc = dispatch_start_utc.isoformat()
            checkpoint.updated_at = datetime.now(UTC).isoformat()
            await write_checkpoint(self._checkpoints_dir, checkpoint)

            result = await self._post_process_phase(
                dispatch, handle, phase_result, worktree_path, dispatch_start_utc
            )

            # Update checkpoint to POST_PROCESSED — crash between here and
            # _write_result_and_nudge will resume at result-write only,
            # avoiding duplicate event emissions from post-processing.
            checkpoint.stage = CheckpointStage.POST_PROCESSED
            checkpoint.updated_at = datetime.now(UTC).isoformat()
            await write_checkpoint(self._checkpoints_dir, checkpoint)

        except Exception as e:
            failed = True
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
            # Record error in checkpoint — keep checkpoint for resume
            checkpoint.last_error = str(e)
            checkpoint.failure_count += 1
            checkpoint.updated_at = datetime.now(UTC).isoformat()
            await write_checkpoint(self._checkpoints_dir, checkpoint)

        # Teardown only on success — preserve VM for resume on failure
        if not failed:
            await self._execution_env.teardown(handle)

        # 4. Write result
        await self._write_result_and_nudge(result, dispatch.workflow_id)

        # Only delete checkpoint on success so failures can be resumed
        if not failed:
            await delete_checkpoint(self._checkpoints_dir, dispatch.workflow_id)

    async def _provision_phase(self, dispatch: Dispatch) -> EnvironmentHandle:
        """Phase 1: Provision execution environment.

        Returns:
            EnvironmentHandle on success.
        """
        handle = await self._execution_env.provision(dispatch, self._config)

        await self._emitter.emit(
            PreflightCompleted(
                timestamp=datetime.now(UTC).isoformat(),
                workflow_id=dispatch.workflow_id,
                passed=True,
                repairs=self._preflight_repairs(handle),
            )
        )

        return handle

    async def _execute_phase(
        self,
        handle: EnvironmentHandle,
        dispatch: Dispatch,
        dispatch_stem: str,
    ) -> PhaseResult:
        """Phase 2: Execute agent/gate process.

        Returns:
            PhaseResult with outcome, signal, and metrics.
        """
        return await self._execution_env.execute(
            handle, dispatch, self._config, dispatch_stem=dispatch_stem
        )

    async def _post_process_phase(
        self,
        dispatch: Dispatch,
        handle: EnvironmentHandle,
        phase_result: PhaseResult,
        worktree_path: Path,
        dispatch_start_utc: datetime,
    ) -> Result:
        """Phase 3: Post-process execution results and construct Result.

        Handles remote sync, gate/tail output, postflight, findings,
        token usage, and PhaseCompleted event emission.

        Returns:
            Fully constructed Result ready for writing.
        """
        duration = phase_result.duration_secs
        spec_folder_path = worktree_path / dispatch.spec_folder

        # Sync remote changes to local worktree for findings parsing
        if self._config.remote_config_path:
            await self._sync_remote_changes(worktree_path)

        # Build gate_output (gate phases only)
        gate_output = None
        if dispatch.phase == Phase.GATE:
            gate_output = _build_gate_output(phase_result.stdout, phase_result.outcome)

        # Build tail_output
        tail_output = None
        is_agent_phase = dispatch.phase not in (
            Phase.GATE,
            Phase.SETUP,
            Phase.CLEANUP,
        )
        if is_agent_phase or phase_result.outcome != Outcome.SUCCESS:
            tail_output = build_tail_output(phase_result.stdout)

        # Apply postflight results
        pushed: bool | None = None
        integrity_repairs = None
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
            spec_modified = postflight_result.integrity_repairs.spec_reverted
            if not postflight_result.pushed and postflight_result.push_error:
                if tail_output:
                    tail_output += f"\n\n--- git push failed ---\n{postflight_result.push_error}"
                else:
                    tail_output = f"git push failed: {postflight_result.push_error}"

        # Capture execution-end timestamp before any post-processing
        exec_end_utc = datetime.now(UTC)

        # Parse structured findings
        new_tasks, findings_data = self._parse_findings(dispatch, spec_folder_path)

        # Collect token usage (best-effort, 30s timeout)
        token_usage_data = None
        if dispatch.cli != Cli.BASH:
            if isinstance(handle.runtime, RemoteEnvironmentRuntime):
                # Already collected inside SSHExecutionEnvironment.execute()
                if phase_result.token_usage is not None:
                    token_usage_data = phase_result.token_usage.model_dump(mode="json")
            else:
                # Local execution: collect here
                runner = LocalCommandRunner()
                usage = await collect_token_usage(
                    dispatch.cli,
                    str(worktree_path),
                    dispatch_start_utc,
                    exec_end_utc,
                    self._config,
                    runner,
                )
                if usage is not None:
                    token_usage_data = usage.model_dump(mode="json")

            if token_usage_data is not None:
                await self._emitter.emit(
                    TokenUsageRecorded(
                        timestamp=datetime.now(UTC).isoformat(),
                        workflow_id=dispatch.workflow_id,
                        phase=dispatch.phase.value,
                        project=dispatch.project,
                        cli=dispatch.cli.value,
                        **{
                            k: v
                            for k, v in token_usage_data.items()
                            if k not in ("provider", "project")
                        },
                    )
                )

        # Construct Result
        result = Result(
            workflow_id=dispatch.workflow_id,
            phase=dispatch.phase,
            outcome=phase_result.outcome,
            signal=phase_result.signal,
            exit_code=phase_result.exit_code,
            duration_secs=duration,
            gate_output=gate_output,
            tail_output=tail_output,
            stderr_tail=build_tail_output(phase_result.stderr),
            unchecked_tasks=phase_result.unchecked_tasks,
            plan_hash=phase_result.plan_hash,
            spec_modified=spec_modified,
            pushed=pushed,
            integrity_repairs=integrity_repairs,
            new_tasks=new_tasks,
            findings=findings_data,
            token_usage=token_usage_data,
        )

        # Emit PhaseCompleted
        await self._emitter.emit(
            PhaseCompleted(
                timestamp=datetime.now(UTC).isoformat(),
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase.value,
                project=dispatch.project,
                outcome=phase_result.outcome.value,
                signal=phase_result.signal,
                duration_secs=duration,
                exit_code=phase_result.exit_code,
            )
        )

        return result

    def _extract_vm_id(self, handle: EnvironmentHandle) -> str | None:
        """Extract VM ID from handle if remote runtime.

        Returns:
            VM ID string, or None for local environments.
        """
        if handle.runtime.kind == "remote":
            remote_rt = cast("RemoteEnvironmentRuntime", handle.runtime)
            return remote_rt.vm_handle.vm_id
        return None

    async def resume_dispatch(self, workflow_id: str) -> Result | None:
        """Resume a dispatch from its most recent checkpoint.

        Returns:
            Result if the dispatch completed, None if no checkpoint found.

        Raises:
            ValueError: If the checkpoint's VM is no longer available.
        """
        checkpoint = await read_checkpoint(self._checkpoints_dir, workflow_id)
        if checkpoint is None:
            return None

        dispatch = Dispatch.model_validate_json(checkpoint.dispatch_json)
        worktree_path = Path(checkpoint.worktree_path)

        # Increment retry count
        checkpoint.retry_count += 1
        checkpoint.updated_at = datetime.now(UTC).isoformat()
        await write_checkpoint(self._checkpoints_dir, checkpoint)

        logger.info(
            "Resuming dispatch %s from stage %s (attempt %d)",
            workflow_id,
            checkpoint.stage,
            checkpoint.retry_count,
        )

        handle: EnvironmentHandle | None = None
        phase_result: PhaseResult | None = None
        result: Result | None = None

        try:
            if checkpoint.stage == CheckpointStage.DISPATCHED:
                # Re-run from provision
                handle = await self._provision_phase(dispatch)
                checkpoint.stage = CheckpointStage.PROVISIONED
                checkpoint.vm_id = self._extract_vm_id(handle)
                checkpoint.environment_profile = dispatch.environment_profile
                if handle.runtime.kind == "remote":
                    remote_rt = cast("RemoteEnvironmentRuntime", handle.runtime)
                    checkpoint.workspace_remote_path = remote_rt.workspace_path.path
                checkpoint.updated_at = datetime.now(UTC).isoformat()
                await write_checkpoint(self._checkpoints_dir, checkpoint)

            if checkpoint.stage == CheckpointStage.PROVISIONED:
                # Skip provision, run execute + post-process
                if handle is None:
                    handle = await self._reconstruct_handle_for_resume(checkpoint, dispatch)
                phase_result = await self._execute_phase(handle, dispatch, checkpoint.dispatch_stem)
                checkpoint.stage = CheckpointStage.EXECUTED
                checkpoint.phase_result_json = phase_result.model_dump_json()
                checkpoint.dispatch_start_utc = datetime.now(UTC).isoformat()
                checkpoint.updated_at = datetime.now(UTC).isoformat()
                await write_checkpoint(self._checkpoints_dir, checkpoint)

            if checkpoint.stage == CheckpointStage.EXECUTED:
                # Skip provision + execute, run post-process only
                if handle is None:
                    handle = await self._reconstruct_handle_for_resume(checkpoint, dispatch)
                if phase_result is None and checkpoint.phase_result_json:
                    phase_result = PhaseResult.model_validate_json(checkpoint.phase_result_json)
                if phase_result is None:
                    raise ValueError("No phase_result available for post-processing")
                dispatch_start_utc = datetime.now(UTC)
                if checkpoint.dispatch_start_utc:
                    dispatch_start_utc = datetime.fromisoformat(checkpoint.dispatch_start_utc)
                result = await self._post_process_phase(
                    dispatch, handle, phase_result, worktree_path, dispatch_start_utc
                )
                checkpoint.stage = CheckpointStage.POST_PROCESSED
                checkpoint.updated_at = datetime.now(UTC).isoformat()
                await write_checkpoint(self._checkpoints_dir, checkpoint)

            if checkpoint.stage == CheckpointStage.POST_PROCESSED:
                logger.info("Checkpoint at POST_PROCESSED stage — writing result only")

            # Write result if available
            if result is not None:
                await self._write_result_and_nudge(result, workflow_id)

        except ProvisionError as e:
            # Provision failed with a terminal result — write it and clean up,
            # matching the handling in _handle_work_phase.
            logger.warning("Resume provision failed for %s: %s", workflow_id, e)
            await self._write_result_and_nudge(e.result, workflow_id)
            await delete_checkpoint(self._checkpoints_dir, workflow_id)
            return e.result

        except Exception as exc:
            logger.exception("Resume failed for %s", workflow_id)
            checkpoint.last_error = str(exc)
            checkpoint.failure_count += 1
            checkpoint.updated_at = datetime.now(UTC).isoformat()
            await write_checkpoint(self._checkpoints_dir, checkpoint)
            # Do NOT teardown on failure — keep VM alive for next resume attempt
            raise

        # Teardown only on success — VM stays alive if resume fails
        if handle is not None:
            await self._execution_env.teardown(handle)

        # Clean up checkpoint only when a result was produced and written
        if result is not None:
            await delete_checkpoint(self._checkpoints_dir, workflow_id)
        return result

    async def _reconstruct_handle_for_resume(
        self,
        checkpoint: Checkpoint,
        dispatch: Dispatch,
    ) -> EnvironmentHandle:
        """Reconstruct an EnvironmentHandle from checkpoint data for resume.

        For remote environments, validates the VM is still active.

        Returns:
            Reconstructed EnvironmentHandle.

        Raises:
            ValueError: If the VM is no longer available.
        """
        worktree_path = Path(checkpoint.worktree_path)

        if checkpoint.vm_id is not None:
            # Remote environment — verify VM still exists
            if not hasattr(self, "_remote_state_store"):
                raise ValueError("No remote state store available for VM verification")

            assignment = await self._remote_state_store.get_assignment(checkpoint.vm_id)
            if assignment is None:
                raise ValueError(
                    f"VM {checkpoint.vm_id} is no longer active — cannot resume. "
                    "Re-dispatch the workflow."
                )

            from tanren_core.adapters.ssh import (  # noqa: PLC0415 — deferred import for optional dependency
                SSHConfig,
                SSHConnection,
            )

            ssh_env = cast("SSHExecutionEnvironment", self._execution_env)
            ssh_config = SSHConfig(
                host=assignment.host,
                user=ssh_env.ssh_defaults.user,
                key_path=ssh_env.ssh_defaults.key_path,
                port=ssh_env.ssh_defaults.port,
                connect_timeout=ssh_env.ssh_defaults.connect_timeout,
                host_key_policy=ssh_env.ssh_defaults.host_key_policy,
            )
            conn = SSHConnection(ssh_config)

            workspace_path = WorkspacePath(
                path=checkpoint.workspace_remote_path or f"/workspace/{dispatch.project}",
                project=dispatch.project,
                branch=dispatch.branch,
            )

            profile = ssh_env._resolve_profile(dispatch, self._config)

            return EnvironmentHandle(
                env_id=f"resume-{checkpoint.workflow_id}",
                worktree_path=worktree_path,
                branch=dispatch.branch,
                project=dispatch.project,
                runtime=RemoteEnvironmentRuntime(
                    vm_handle=VMHandle(
                        vm_id=checkpoint.vm_id,
                        host=assignment.host,
                        provider=ssh_env._provider,
                        created_at=assignment.assigned_at,
                    ),
                    connection=conn,
                    workspace_path=workspace_path,
                    profile=profile,
                    teardown_commands=profile.teardown,
                    provision_start=time.monotonic(),
                    workflow_id=dispatch.workflow_id,
                ),
            )

        # Local environment — re-run env validation to rebuild task_env
        env_report, task_env = await self._env_validator.load_and_validate(worktree_path)
        return EnvironmentHandle(
            env_id=f"resume-{checkpoint.workflow_id}",
            worktree_path=worktree_path,
            branch=dispatch.branch,
            project=dispatch.project,
            runtime=LocalEnvironmentRuntime(
                task_env=task_env,
                env_report=env_report,
            ),
        )

    async def _recover_checkpoints(self) -> None:
        """Resume any checkpointed dispatches found at startup.

        Best-effort: logs errors but does not crash the worker.
        """
        checkpoints = await list_checkpoints(self._checkpoints_dir)
        if not checkpoints:
            return

        logger.info("Found %d checkpoint(s) to resume at startup", len(checkpoints))
        for cp in checkpoints:
            try:
                await self.resume_dispatch(cp.workflow_id)
                logger.info("Successfully resumed dispatch %s", cp.workflow_id)
            except Exception:
                logger.warning(
                    "Failed to resume dispatch %s — checkpoint retained for manual retry",
                    cp.workflow_id,
                    exc_info=True,
                )

    async def _get_checkpoint_vm_ids(self) -> frozenset[str]:
        """Collect VM IDs referenced by active checkpoints.

        Returns:
            Set of VM IDs that should not be released by stale VM cleanup.
        """
        checkpoints = await list_checkpoints(self._checkpoints_dir)
        return frozenset(cp.vm_id for cp in checkpoints if cp.vm_id is not None)

    def _parse_findings(
        self, dispatch: Dispatch, spec_folder_path: Path
    ) -> tuple[list[dict[str, JsonValue]], list[dict[str, JsonValue]]]:
        """Parse structured findings from audit/demo/investigate phases.

        Returns:
            Tuple of (new_tasks, findings_data) dicts.
        """
        new_tasks: list[dict[str, JsonValue]] = []
        findings_data: list[dict[str, JsonValue]] = []

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
                findings_data = [{"report": report.model_dump(mode="json")}]
                for rc in report.root_causes:
                    new_tasks.extend(rc.suggested_tasks)

        return new_tasks, findings_data

    def _preflight_repairs(self, handle: EnvironmentHandle) -> list[str]:
        """Extract preflight repairs from local runtime handles.

        Returns:
            List of repair description strings.
        """
        if handle.runtime.kind != "local":
            return []
        local_runtime = cast("LocalEnvironmentRuntime", handle.runtime)
        if local_runtime.preflight_result is None:
            return []
        return local_runtime.preflight_result.repairs

    async def _sync_remote_changes(self, worktree_path: Path) -> None:
        """Pull remote changes into local worktree after remote execution."""
        try:
            result = await asyncio.to_thread(
                subprocess.run,
                ["git", "pull", "--ff-only"],
                cwd=worktree_path,
                capture_output=True,
                text=True,
                timeout=60,
                check=False,
            )
            if result.returncode != 0:
                logger.warning(
                    "git pull failed in worktree %s: %s",
                    worktree_path,
                    result.stderr.strip(),
                )
        except Exception:
            logger.warning(
                "Failed to sync remote changes to %s",
                worktree_path,
                exc_info=True,
            )

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
