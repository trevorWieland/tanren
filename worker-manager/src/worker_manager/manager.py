"""Top-level worker manager: poll loop, dispatch handling, startup/shutdown."""

import asyncio
import contextlib
import logging
import os
import signal
from datetime import UTC, datetime
from pathlib import Path

from worker_manager.config import Config
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
from worker_manager.postflight import run_postflight
from worker_manager.preflight import run_preflight
from worker_manager.process import spawn_process
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
from worker_manager.worktree import (
    cleanup_worktree,
    create_worktree,
    register_worktree,
)

logger = logging.getLogger(__name__)

_PUSH_PHASES = frozenset({Phase.DO_TASK, Phase.AUDIT_TASK, Phase.RUN_DEMO, Phase.AUDIT_SPEC})


class WorkerManager:
    """Main worker manager service."""

    def __init__(self, config: Config | None = None) -> None:
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

        except Exception:
            logger.exception("Unhandled error in dispatch %s", dispatch.workflow_id)
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
            wt_path = await create_worktree(
                dispatch.project, issue, dispatch.branch, self._config.github_dir
            )
            await register_worktree(
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
            await cleanup_worktree(
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

        # Pre-flight checks (replaces validate_worktree + manual snapshot + status clear)
        preflight = await run_preflight(
            worktree_path, dispatch.branch, spec_folder_path, dispatch.phase.value
        )
        if not preflight.passed:
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=-1,
                duration_secs=0,
                gate_output=None,
                tail_output=preflight.error,
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )
            await self._write_result_and_nudge(result, dispatch.workflow_id)
            return

        if preflight.repairs:
            logger.info("Preflight repairs: %s", preflight.repairs)

        # Start heartbeat
        await self._heartbeat.start(dispatch_stem)

        start = time.monotonic()
        transient_retries = 0
        try:
            while True:
                # Spawn process
                proc_result = await spawn_process(dispatch, worktree_path, self._config)

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
                outcome, signal = map_outcome(
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
                        signal,
                    )
                    if (
                        error_class == ErrorClass.TRANSIENT
                        and transient_retries < 3
                    ):
                        transient_retries += 1
                        logger.warning(
                            "Transient error (attempt %d/3), retrying in %ds",
                            transient_retries,
                            TRANSIENT_BACKOFF[transient_retries - 1],
                        )
                        await asyncio.sleep(
                            TRANSIENT_BACKOFF[transient_retries - 1]
                        )
                        continue
                    elif (
                        error_class == ErrorClass.AMBIGUOUS
                        and transient_retries < 1
                    ):
                        transient_retries += 1
                        logger.warning(
                            "Ambiguous error, retrying once in 10s"
                        )
                        await asyncio.sleep(10)
                        continue

                # Not retrying — break out of loop
                break

            duration = int(time.monotonic() - start)

            # Compute plan.md metrics (backward compat for coordinator)
            plan_path = spec_folder_path / "plan.md"
            unchecked = await count_unchecked_tasks(plan_path)
            plan_hash = await compute_plan_hash(plan_path)

            # Build gate_output (last 100 lines for gate phases)
            gate_output = None
            if dispatch.phase == Phase.GATE and proc_result.stdout:
                lines = proc_result.stdout.strip().split("\n")
                gate_output = "\n".join(lines[-100:])

            # Build tail_output — always for agent phases (operational visibility),
            # only on non-success for gate/setup/cleanup phases.
            tail_output = None
            is_agent_phase = dispatch.phase not in (
                Phase.GATE, Phase.SETUP, Phase.CLEANUP,
            )
            if (
                is_agent_phase or outcome != Outcome.SUCCESS
            ) and proc_result.stdout:
                lines = proc_result.stdout.strip().split("\n")
                tail_output = "\n".join(lines[-50:])

            # Post-flight integrity checks (always run — skip push for errors/timeouts)
            pushed: bool | None = None
            integrity_repairs: dict | None = None
            spec_modified = False
            if dispatch.phase in _PUSH_PHASES:
                postflight = await run_postflight(
                    worktree_path,
                    dispatch.branch,
                    dispatch.phase.value,
                    preflight.file_hashes,
                    preflight.file_backups,
                    skip_push=(outcome in (Outcome.ERROR, Outcome.TIMEOUT)),
                )
                pushed = postflight.pushed
                integrity_repairs = postflight.integrity_repairs
                spec_modified = postflight.integrity_repairs.get(
                    "spec_reverted", False
                )
                if not postflight.pushed and postflight.push_error:
                    if tail_output:
                        tail_output += (
                            f"\n\n--- git push failed ---\n"
                            f"{postflight.push_error}"
                        )
                    else:
                        tail_output = (
                            f"git push failed: {postflight.push_error}"
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
                signal=signal,
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
