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
    PhaseStarted,
    PostflightCompleted,
    PreflightCompleted,
    RetryScheduled,
    SqliteEventEmitter,
    SubprocessSpawner,
)
from worker_manager.adapters.protocols import (
    EnvProvisioner,
    EnvValidator,
    EventEmitter,
    PostflightRunner,
    PreflightRunner,
    ProcessSpawner,
    WorktreeManager,
)
from worker_manager.config import Config
from worker_manager.env.reporter import format_report
from worker_manager.errors import TRANSIENT_BACKOFF, ErrorClass, classify_error
from worker_manager.heartbeat import HeartbeatWriter
from worker_manager.ipc import (
    atomic_write,
    delete_file,
    scan_dispatch_dir,
    write_nudge,
    write_result,
)
from worker_manager.metrics import (
    compute_plan_hash,
    count_unchecked_tasks,
)
from worker_manager.queues import DispatchRouter
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
    extract_signal,
    map_outcome,
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
    """Main worker manager service."""

    def __init__(
        self,
        config: Config | None = None,
        *,
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
            max_opencode=self._config.max_opencode,
            max_codex=self._config.max_codex,
            max_gate=self._config.max_gate,
        )

        # Adapters — default to concrete implementations when not injected
        self._worktree_mgr = worktree_mgr or GitWorktreeManager()
        self._preflight = preflight or GitPreflightRunner()
        self._postflight = postflight or GitPostflightRunner()
        self._spawner = spawner or SubprocessSpawner()
        self._env_validator = env_validator or DotenvEnvValidator()
        self._env_provisioner = env_provisioner or DotenvEnvProvisioner()

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

    async def _handle_dispatch(self, path: Path, dispatch: Dispatch) -> None:
        """Handle a single dispatch through its full lifecycle."""
        dispatch_stem = path.stem
        issue = parse_issue_from_workflow_id(dispatch.workflow_id)
        worktree_path = Path(self._config.github_dir) / f"{dispatch.project}-wt-{issue}"

        now = datetime.now(UTC).isoformat()

        # Emit DispatchReceived
        await self._emitter.emit(DispatchReceived(
            timestamp=now,
            workflow_id=dispatch.workflow_id,
            phase=dispatch.phase.value,
            project=dispatch.project,
            cli=dispatch.cli.value,
        ))

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

            await self._emitter.emit(ErrorOccurred(
                timestamp=datetime.now(UTC).isoformat(),
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase.value,
                error=str(exc),
                error_class=None,
            ))

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

    async def _handle_setup(
        self, dispatch: Dispatch, issue: int, worktree_path: Path
    ) -> None:
        """Handle setup phase: create worktree + register."""
        import time

        start = time.monotonic()
        try:
            wt_path = await self._worktree_mgr.create(
                dispatch.project, issue, dispatch.branch, self._config.github_dir
            )
            project_dir = Path(self._config.github_dir) / dispatch.project
            count = await asyncio.to_thread(
                self._env_provisioner.provision, wt_path, project_dir
            )
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
        """Handle agent/gate phases: preflight, spawn, postflight, report."""
        import time

        spec_folder_path = worktree_path / dispatch.spec_folder

        # Environment preflight validation
        env_report, task_env = await self._env_validator.load_and_validate(worktree_path)
        if not env_report.passed:
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=-1,
                duration_secs=0,
                gate_output=None,
                tail_output=format_report(
                    env_report, dispatch.project, str(worktree_path / "tanren.yml")
                ),
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )
            await self._write_result_and_nudge(result, dispatch.workflow_id)
            return

        # Pre-flight checks (replaces validate_worktree + manual snapshot + status clear)
        preflight_result = await self._preflight.run(
            worktree_path, dispatch.branch, spec_folder_path, dispatch.phase.value
        )

        await self._emitter.emit(PreflightCompleted(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch.workflow_id,
            passed=preflight_result.passed,
            repairs=preflight_result.repairs,
        ))

        if not preflight_result.passed:
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=-1,
                duration_secs=0,
                gate_output=None,
                tail_output=preflight_result.error,
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )
            await self._write_result_and_nudge(result, dispatch.workflow_id)
            return

        if preflight_result.repairs:
            logger.info("Preflight repairs: %s", preflight_result.repairs)

        # Start heartbeat
        await self._heartbeat.start(dispatch_stem)

        start = time.monotonic()
        transient_retries = 0
        try:
            while True:
                # Emit PhaseStarted before spawn
                await self._emitter.emit(PhaseStarted(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    phase=dispatch.phase.value,
                    worktree_path=str(worktree_path),
                ))

                # Spawn process
                proc_result = await self._spawner.spawn(
                    dispatch, worktree_path, self._config, task_env=task_env or None
                )

                # Always log process result for agent phases
                if dispatch.phase not in (Phase.GATE, Phase.SETUP, Phase.CLEANUP):
                    stdout_preview = (proc_result.stdout or "")[:500]
                    logger.info(
                        "Process result: exit=%d duration=%ds "
                        "timed_out=%s stdout_len=%d stdout=%.200s",
                        proc_result.exit_code,
                        proc_result.duration_secs,
                        proc_result.timed_out,
                        len(proc_result.stdout or ""),
                        stdout_preview,
                    )

                # Extract signal
                command_name = dispatch.phase.value
                raw_signal = extract_signal(
                    dispatch.phase, command_name, spec_folder_path, proc_result.stdout
                )

                # Map outcome
                outcome, signal_val = map_outcome(
                    dispatch.phase,
                    raw_signal,
                    proc_result.exit_code,
                    proc_result.timed_out,
                )

                # Transient error retry
                if outcome in (Outcome.ERROR, Outcome.TIMEOUT):
                    stderr_text = ""
                    error_class = classify_error(
                        proc_result.exit_code,
                        proc_result.stdout or "",
                        stderr_text,
                        signal_val,
                    )
                    if (
                        error_class == ErrorClass.TRANSIENT
                        and transient_retries < 3
                    ):
                        transient_retries += 1
                        backoff = TRANSIENT_BACKOFF[transient_retries - 1]
                        logger.warning(
                            "Transient error (attempt %d/3), retrying in %ds",
                            transient_retries,
                            backoff,
                        )
                        await self._emitter.emit(RetryScheduled(
                            timestamp=datetime.now(UTC).isoformat(),
                            workflow_id=dispatch.workflow_id,
                            phase=dispatch.phase.value,
                            attempt=transient_retries,
                            max_attempts=3,
                            backoff_secs=backoff,
                        ))
                        await asyncio.sleep(backoff)
                        continue
                    elif (
                        error_class == ErrorClass.AMBIGUOUS
                        and transient_retries < 1
                    ):
                        transient_retries += 1
                        logger.warning(
                            "Ambiguous error, retrying once in 10s"
                        )
                        await self._emitter.emit(RetryScheduled(
                            timestamp=datetime.now(UTC).isoformat(),
                            workflow_id=dispatch.workflow_id,
                            phase=dispatch.phase.value,
                            attempt=transient_retries,
                            max_attempts=1,
                            backoff_secs=10,
                        ))
                        await asyncio.sleep(10)
                        continue

                # Not retrying — break out of loop
                break

            duration = int(time.monotonic() - start)

            # Compute plan.md metrics (backward compat for coordinator)
            plan_path = spec_folder_path / "plan.md"
            unchecked = await count_unchecked_tasks(plan_path)
            plan_hash = await compute_plan_hash(plan_path)

            # Build gate_output (gate phases only)
            gate_output = None
            if dispatch.phase == Phase.GATE:
                gate_output = _build_gate_output(proc_result.stdout, outcome)

            # Build tail_output — always for agent phases (operational visibility),
            # only on non-success for gate/setup/cleanup phases.
            tail_output = None
            is_agent_phase = dispatch.phase not in (
                Phase.GATE, Phase.SETUP, Phase.CLEANUP,
            )
            if is_agent_phase or outcome != Outcome.SUCCESS:
                tail_output = _build_tail_output(proc_result.stdout)

            # Post-flight integrity checks (always run — skip push for errors/timeouts)
            pushed: bool | None = None
            integrity_repairs: dict | None = None
            spec_modified = False
            if dispatch.phase in _PUSH_PHASES:
                postflight_result = await self._postflight.run(
                    worktree_path,
                    dispatch.branch,
                    dispatch.phase.value,
                    preflight_result.file_hashes,
                    preflight_result.file_backups,
                    skip_push=(outcome in (Outcome.ERROR, Outcome.TIMEOUT)),
                )

                await self._emitter.emit(PostflightCompleted(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    phase=dispatch.phase.value,
                    pushed=postflight_result.pushed,
                    integrity_repairs=postflight_result.integrity_repairs,
                ))

                pushed = postflight_result.pushed
                integrity_repairs = postflight_result.integrity_repairs
                spec_modified = postflight_result.integrity_repairs.get(
                    "spec_reverted", False
                )
                if not postflight_result.pushed and postflight_result.push_error:
                    if tail_output:
                        tail_output += (
                            f"\n\n--- git push failed ---\n"
                            f"{postflight_result.push_error}"
                        )
                    else:
                        tail_output = (
                            f"git push failed: {postflight_result.push_error}"
                        )

            # Parse structured findings
            new_tasks: list[dict] = []
            findings_data: list[dict] = []

            if dispatch.phase == Phase.AUDIT_TASK:
                findings = parse_audit_findings(spec_folder_path)
                if findings:
                    findings_data = [f.model_dump() for f in findings.findings]
                    new_tasks = [
                        f.model_dump()
                        for f in findings.findings
                        if f.severity == FindingSeverity.FIX
                    ]
            elif dispatch.phase == Phase.RUN_DEMO:
                findings = parse_demo_findings(spec_folder_path)
                if findings:
                    findings_data = [f.model_dump() for f in findings.findings]
                    new_tasks = [
                        f.model_dump()
                        for f in findings.findings
                        if f.severity == FindingSeverity.FIX
                    ]
            elif dispatch.phase == Phase.AUDIT_SPEC:
                spec_findings = parse_audit_spec_findings(spec_folder_path)
                if spec_findings:
                    findings_data = [f.model_dump() for f in spec_findings]
                    new_tasks = [
                        f.model_dump()
                        for f in spec_findings
                        if f.severity == FindingSeverity.FIX
                    ]
            elif dispatch.phase == Phase.INVESTIGATE:
                report = parse_investigation_report(spec_folder_path)
                if report:
                    findings_data = [{"report": report.model_dump()}]
                    for rc in report.root_causes:
                        new_tasks.extend(rc.suggested_tasks)

            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=outcome,
                signal=signal_val,
                exit_code=proc_result.exit_code,
                duration_secs=duration,
                gate_output=gate_output,
                tail_output=tail_output,
                unchecked_tasks=unchecked,
                plan_hash=plan_hash,
                spec_modified=spec_modified,
                pushed=pushed,
                integrity_repairs=integrity_repairs,
                new_tasks=new_tasks,
                findings=findings_data,
            )

            # Emit PhaseCompleted before writing result
            await self._emitter.emit(PhaseCompleted(
                timestamp=datetime.now(UTC).isoformat(),
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase.value,
                outcome=outcome.value,
                signal=signal_val,
                duration_secs=duration,
                exit_code=proc_result.exit_code,
            ))

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
            # Stop heartbeat
            await self._heartbeat.stop(dispatch_stem)

        # Write result + nudge
        await self._write_result_and_nudge(result, dispatch.workflow_id)

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
