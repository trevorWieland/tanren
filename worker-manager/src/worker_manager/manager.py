"""Top-level worker manager: poll loop, dispatch handling, startup/shutdown."""

import asyncio
import contextlib
import logging
import signal
from pathlib import Path

from worker_manager.config import Config
from worker_manager.heartbeat import HeartbeatWriter
from worker_manager.ipc import delete_file, scan_dispatch_dir, write_nudge, write_result
from worker_manager.metrics import (
    compute_plan_hash,
    count_unchecked_tasks,
    guard_spec_md,
    snapshot_spec,
)
from worker_manager.process import spawn_process
from worker_manager.queues import DispatchRouter
from worker_manager.schemas import (
    Dispatch,
    Nudge,
    Outcome,
    Phase,
    Result,
    parse_issue_from_workflow_id,
)
from worker_manager.signals import extract_signal, map_outcome
from worker_manager.worktree import (
    cleanup_worktree,
    create_worktree,
    register_worktree,
    validate_worktree,
)

logger = logging.getLogger(__name__)


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

                await asyncio.sleep(self._config.poll_interval)
        except asyncio.CancelledError:
            pass

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
        """Handle agent/gate phases: validate, spawn, extract, report."""
        import time

        # Validate worktree
        await validate_worktree(worktree_path, dispatch.branch)

        spec_folder_path = worktree_path / dispatch.spec_folder

        # Snapshot spec.md + backup (for integrity guard)
        spec_path = spec_folder_path / "spec.md"
        original_md5, backup_content = await snapshot_spec(spec_path)

        # Clear .agent-status file
        status_file = spec_folder_path / ".agent-status"
        if status_file.exists():
            status_file.unlink()

        # Start heartbeat
        await self._heartbeat.start(dispatch_stem)

        start = time.monotonic()
        try:
            # Spawn process
            proc_result = await spawn_process(dispatch, worktree_path, self._config)

            # Extract signal
            command_name = dispatch.phase.value
            raw_signal = extract_signal(
                dispatch.phase, command_name, spec_folder_path, proc_result.stdout
            )

            # Guard spec.md integrity
            spec_modified = await guard_spec_md(spec_path, original_md5, backup_content)

            # Compute plan.md metrics
            plan_path = spec_folder_path / "plan.md"
            unchecked = await count_unchecked_tasks(plan_path)
            plan_hash = await compute_plan_hash(plan_path)

            # Map outcome
            outcome, signal = map_outcome(
                dispatch.phase, raw_signal, proc_result.exit_code, proc_result.timed_out
            )

            duration = int(time.monotonic() - start)

            # Build gate_output (last 100 lines for gate phases)
            gate_output = None
            if dispatch.phase == Phase.GATE and proc_result.stdout:
                lines = proc_result.stdout.strip().split("\n")
                gate_output = "\n".join(lines[-100:])

            # Build tail_output (last 50 lines for non-success)
            tail_output = None
            if outcome != Outcome.SUCCESS and proc_result.stdout:
                lines = proc_result.stdout.strip().split("\n")
                tail_output = "\n".join(lines[-50:])

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
