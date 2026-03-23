"""Queue-consuming worker that processes dispatch steps.

Each step (provision, execute, teardown) is consumed from the
``JobQueue``, executed via ``ExecutionEnvironment``, and acknowledged
with its result.

Auto-chained dispatches (``DispatchMode.AUTO``) automatically enqueue
the next step in the same transaction as the completion event.
"""

from __future__ import annotations

import asyncio
import logging
import os
import time
import uuid
from datetime import UTC, datetime
from typing import TYPE_CHECKING, TypedDict

from tanren_core.adapters.events import (
    ErrorOccurred,
    PhaseCompleted,
    PreflightCompleted,
    TokenUsageRecorded,
    VMProvisioned,
    VMReleased,
)
from tanren_core.adapters.types import EnvironmentHandle, ProvisionError
from tanren_core.errors import ErrorClass, classify_error
from tanren_core.schemas import Outcome
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
from tanren_core.store.events import (
    DispatchCompleted,
    DispatchFailed,
    StepCompleted,
    StepFailed,
    StepStarted,
)
from tanren_core.store.handle import PersistedEnvironmentHandle
from tanren_core.store.payloads import (
    DryRunResult,
    DryRunStepPayload,
    ExecuteResult,
    ExecuteStepPayload,
    ProvisionResult,
    ProvisionStepPayload,
    TeardownResult,
    TeardownStepPayload,
)
from tanren_core.store.views import QueuedStep

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import ExecutionEnvironment
    from tanren_core.schemas import Dispatch
    from tanren_core.store.protocols import EventStore, JobQueue, StateStore
    from tanren_core.worker_config import WorkerConfig

logger = logging.getLogger(__name__)


class _NextStepKwargs(TypedDict):
    """Keyword arguments for ``ack_and_enqueue()`` (next step fields)."""

    next_step_id: str
    next_dispatch_id: str
    next_step_type: str
    next_step_sequence: int
    next_lane: str | None
    next_payload_json: str


_MAX_RETRIES = 3
_TRANSIENT_BACKOFF = (10, 30, 60)


class Worker:
    """Queue-consuming worker that processes dispatch steps."""

    def __init__(
        self,
        *,
        config: WorkerConfig,
        event_store: EventStore,
        job_queue: JobQueue,
        state_store: StateStore,
        execution_env: ExecutionEnvironment,
    ) -> None:
        """Initialise the worker with its dependencies."""
        self._config = config
        self._event_store = event_store
        self._job_queue = job_queue
        self._state_store = state_store
        self._execution_env = execution_env
        self._shutdown = asyncio.Event()
        self._worker_id = config.worker_id or f"{os.uname().nodename}-{os.getpid()}"

    async def run(self) -> None:
        """Start lane consumers and run until shutdown."""
        tasks = [
            asyncio.create_task(
                self._lane_consumer(None, self._config.max_provision),
                name="infra-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.IMPL, self._config.max_impl),
                name="impl-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.AUDIT, self._config.max_audit),
                name="audit-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.GATE, self._config.max_gate),
                name="gate-consumer",
            ),
        ]
        try:
            await asyncio.gather(*tasks)
        except asyncio.CancelledError:
            pass
        finally:
            for t in tasks:
                t.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)

    async def run_until_dispatch_complete(self, dispatch_id: str) -> None:
        """Run the worker loop until a specific dispatch reaches terminal state.

        Used by the CLI's embedded worker.
        """
        tasks = [
            asyncio.create_task(
                self._lane_consumer(None, self._config.max_provision),
                name="infra-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.IMPL, self._config.max_impl),
                name="impl-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.AUDIT, self._config.max_audit),
                name="audit-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.GATE, self._config.max_gate),
                name="gate-consumer",
            ),
        ]

        try:
            while not self._shutdown.is_set():
                view = await self._state_store.get_dispatch(dispatch_id)
                if view and view.status in (
                    DispatchStatus.COMPLETED,
                    DispatchStatus.FAILED,
                    DispatchStatus.CANCELLED,
                ):
                    self._shutdown.set()
                    break
                await asyncio.sleep(0.5)
        finally:
            for t in tasks:
                t.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)

    async def run_until_step_complete(self, dispatch_id: str, step_type: StepType) -> None:
        """Run the worker loop until a specific step type reaches terminal state.

        Used by the CLI's embedded worker for individual lifecycle steps
        (provision, execute, teardown).
        """
        tasks = [
            asyncio.create_task(
                self._lane_consumer(None, self._config.max_provision),
                name="infra-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.IMPL, self._config.max_impl),
                name="impl-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.AUDIT, self._config.max_audit),
                name="audit-consumer",
            ),
            asyncio.create_task(
                self._lane_consumer(Lane.GATE, self._config.max_gate),
                name="gate-consumer",
            ),
        ]

        try:
            while not self._shutdown.is_set():
                steps = await self._state_store.get_steps_for_dispatch(dispatch_id)
                matching = next(
                    (s for s in steps if s.step_type == step_type),
                    None,
                )
                if matching and matching.status in (
                    StepStatus.COMPLETED,
                    StepStatus.FAILED,
                ):
                    self._shutdown.set()
                    break
                await asyncio.sleep(0.5)
        finally:
            for t in tasks:
                t.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)

    def shutdown(self) -> None:
        """Signal all consumers to stop."""
        self._shutdown.set()

    # ── Lane consumer loop ────────────────────────────────────────────────

    async def _lane_consumer(
        self,
        lane: Lane | None,
        max_concurrent: int,
    ) -> None:
        """Poll the queue for steps matching the given lane."""
        lane_name = str(lane) if lane else "infra"
        logger.info(
            "Worker %s starting %s consumer (max=%d)", self._worker_id, lane_name, max_concurrent
        )

        while not self._shutdown.is_set():
            step = await self._job_queue.dequeue(
                lane=lane,
                worker_id=self._worker_id,
                max_concurrent=max_concurrent,
            )
            if step is None:
                try:
                    await asyncio.wait_for(
                        self._shutdown.wait(),
                        timeout=self._config.poll_interval_secs,
                    )
                except TimeoutError:
                    pass
                continue

            try:
                await self.process_step(step)
            except Exception:
                logger.exception(
                    "Unhandled error processing step %s (dispatch %s)",
                    step.step_id,
                    step.dispatch_id,
                )

    # ── Step dispatch ─────────────────────────────────────────────────────

    async def process_step(self, step: QueuedStep) -> None:
        """Route a step to the appropriate handler."""
        try:
            now = datetime.now(UTC).isoformat()
            await self._event_store.append(
                StepStarted(
                    timestamp=now,
                    workflow_id=step.dispatch_id,
                    step_id=step.step_id,
                    worker_id=self._worker_id,
                    step_type=step.step_type,
                )
            )

            if step.step_type == StepType.PROVISION:
                payload = ProvisionStepPayload.model_validate_json(step.payload_json)
                await self._do_provision(payload, step)
            elif step.step_type == StepType.EXECUTE:
                payload_exec = ExecuteStepPayload.model_validate_json(step.payload_json)
                await self._do_execute(payload_exec, step)
            elif step.step_type == StepType.TEARDOWN:
                payload_td = TeardownStepPayload.model_validate_json(step.payload_json)
                await self._do_teardown(payload_td, step)
            elif step.step_type == StepType.DRY_RUN:
                payload_dr = DryRunStepPayload.model_validate_json(step.payload_json)
                await self._do_dry_run(payload_dr, step)
            else:
                raise ValueError(f"Unknown step type: {step.step_type}")
        except Exception as exc:
            await self._handle_step_failure(step, exc)

    # ── Provision ─────────────────────────────────────────────────────────

    async def _do_provision(
        self,
        payload: ProvisionStepPayload,
        step: QueuedStep,
    ) -> None:
        """Execute a provision step."""
        start = time.monotonic()
        dispatch = payload.dispatch

        try:
            handle = await self._execution_env.provision(dispatch, self._config)
        except ProvisionError:
            duration = int(time.monotonic() - start)
            await self._event_store.append(
                PreflightCompleted(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    passed=False,
                    repairs=[],
                )
            )
            raise

        duration = int(time.monotonic() - start)
        persisted = self._persist_handle(handle, dispatch_id=dispatch.workflow_id)
        result = ProvisionResult(handle=persisted)

        # Emit VMProvisioned event for remote environments
        if persisted.vm is not None:
            await self._event_store.append(
                VMProvisioned(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    vm_id=persisted.vm.vm_id,
                    host=persisted.vm.host,
                    provider=persisted.vm.provider,
                    project=dispatch.project,
                    profile=dispatch.environment_profile,
                    hourly_cost=persisted.vm.hourly_cost,
                )
            )

        step_completed_event = StepCompleted(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch.workflow_id,
            step_id=step.step_id,
            step_type=StepType.PROVISION,
            duration_secs=duration,
            result_payload=result,
        )

        # Auto-chain: atomically ack + enqueue execute step
        dispatch_view = await self._state_store.get_dispatch(dispatch.workflow_id)
        if dispatch_view and dispatch_view.mode == DispatchMode.AUTO:
            next_step = self._build_next_step(
                dispatch=dispatch,
                next_type=StepType.EXECUTE,
                next_sequence=1,
                handle=persisted,
            )
            if next_step is not None:
                await self._job_queue.ack_and_enqueue(
                    step.step_id,
                    result_json=result.model_dump_json(),
                    completion_events=[step_completed_event],
                    **next_step,
                )
            else:
                await self._event_store.append(step_completed_event)
                await self._job_queue.ack(step.step_id, result_json=result.model_dump_json())
        else:
            await self._event_store.append(step_completed_event)
            await self._job_queue.ack(step.step_id, result_json=result.model_dump_json())

    # ── Execute ───────────────────────────────────────────────────────────

    async def _do_execute(
        self,
        payload: ExecuteStepPayload,
        step: QueuedStep,
    ) -> None:
        """Execute an execute step."""
        start = time.monotonic()
        dispatch = payload.dispatch
        handle = self._reconstruct_handle(payload.handle)

        phase_result = await self._execution_env.execute(
            handle,
            dispatch,
            self._config,
        )
        duration = int(time.monotonic() - start)

        result = ExecuteResult(
            outcome=phase_result.outcome,
            signal=phase_result.signal,
            exit_code=phase_result.exit_code,
            duration_secs=duration,
            gate_output=phase_result.gate_output,
            pushed=phase_result.postflight_result.pushed
            if phase_result.postflight_result
            else None,
            plan_hash=phase_result.plan_hash,
            unchecked_tasks=phase_result.unchecked_tasks,
            spec_modified=phase_result.postflight_result.integrity_repairs.spec_reverted
            if phase_result.postflight_result and phase_result.postflight_result.integrity_repairs
            else False,
            token_usage=phase_result.token_usage,
        )

        # Emit token usage event if usage data was collected
        if phase_result.token_usage is not None:
            tu = phase_result.token_usage
            await self._event_store.append(
                TokenUsageRecorded(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    phase=str(dispatch.phase),
                    project=dispatch.project,
                    cli=str(dispatch.cli),
                    input_tokens=tu.input_tokens,
                    output_tokens=tu.output_tokens,
                    cache_creation_tokens=getattr(tu, "cache_creation_tokens", 0),
                    cache_read_tokens=getattr(tu, "cache_read_tokens", 0),
                    cached_input_tokens=getattr(tu, "cached_input_tokens", 0),
                    reasoning_tokens=getattr(tu, "reasoning_tokens", 0),
                    total_tokens=tu.total_tokens,
                    total_cost=tu.total_cost,
                    models_used=list(getattr(tu, "models_used", [])),
                    session_id=getattr(tu, "session_id", None),
                )
            )

        step_completed_event = StepCompleted(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch.workflow_id,
            step_id=step.step_id,
            step_type=StepType.EXECUTE,
            duration_secs=duration,
            result_payload=result,
        )
        phase_completed_event = PhaseCompleted(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch.workflow_id,
            phase=str(dispatch.phase),
            project=dispatch.project,
            outcome=str(phase_result.outcome),
            signal=phase_result.signal,
            duration_secs=duration,
            exit_code=phase_result.exit_code,
        )

        # Auto-chain: atomically ack + enqueue teardown (respecting preserve_on_failure)
        dispatch_view = await self._state_store.get_dispatch(dispatch.workflow_id)
        if dispatch_view and dispatch_view.mode == DispatchMode.AUTO:
            if (
                phase_result.outcome in (Outcome.ERROR, Outcome.TIMEOUT)
                and dispatch.preserve_on_failure
            ):
                # Terminal failure with preserve — ack then mark dispatch failed, skip teardown
                await self._event_store.append(step_completed_event)
                await self._event_store.append(phase_completed_event)
                await self._job_queue.ack(step.step_id, result_json=result.model_dump_json())
                await self._mark_dispatch_failed(
                    dispatch.workflow_id,
                    step.step_id,
                    StepType.EXECUTE,
                    f"Execution {phase_result.outcome}, VM preserved",
                )
            else:
                preserve = dispatch.preserve_on_failure and phase_result.outcome in (
                    Outcome.ERROR,
                    Outcome.TIMEOUT,
                )
                next_step = self._build_next_step(
                    dispatch=dispatch,
                    next_type=StepType.TEARDOWN,
                    next_sequence=2,
                    handle=payload.handle,
                    preserve=preserve,
                )
                if next_step is not None:
                    await self._job_queue.ack_and_enqueue(
                        step.step_id,
                        result_json=result.model_dump_json(),
                        completion_events=[step_completed_event, phase_completed_event],
                        **next_step,
                    )
                else:
                    await self._event_store.append(step_completed_event)
                    await self._event_store.append(phase_completed_event)
                    await self._job_queue.ack(step.step_id, result_json=result.model_dump_json())
        else:
            await self._event_store.append(step_completed_event)
            await self._event_store.append(phase_completed_event)
            await self._job_queue.ack(step.step_id, result_json=result.model_dump_json())

    # ── Teardown ──────────────────────────────────────────────────────────

    async def _do_teardown(
        self,
        payload: TeardownStepPayload,
        step: QueuedStep,
    ) -> None:
        """Execute a teardown step."""
        start = time.monotonic()
        dispatch = payload.dispatch

        if not payload.preserve:
            handle = self._reconstruct_handle(payload.handle)
            await self._execution_env.teardown(handle)

        duration = int(time.monotonic() - start)
        result = TeardownResult(
            vm_released=not payload.preserve,
            duration_secs=duration,
        )

        # Emit VMReleased event for remote environments
        if not payload.preserve and payload.handle.vm is not None:
            provision_ts = payload.handle.provision_timestamp
            vm_duration = duration
            try:
                created = datetime.fromisoformat(provision_ts)
                vm_duration = int((datetime.now(UTC) - created).total_seconds())
            except ValueError, TypeError:
                pass
            hourly = payload.handle.vm.hourly_cost
            estimated_cost = (hourly * vm_duration / 3600.0) if hourly else None
            await self._event_store.append(
                VMReleased(
                    timestamp=datetime.now(UTC).isoformat(),
                    workflow_id=dispatch.workflow_id,
                    vm_id=payload.handle.vm.vm_id,
                    project=dispatch.project,
                    duration_secs=vm_duration,
                    estimated_cost=estimated_cost,
                )
            )

        await self._event_store.append(
            StepCompleted(
                timestamp=datetime.now(UTC).isoformat(),
                workflow_id=dispatch.workflow_id,
                step_id=step.step_id,
                step_type=StepType.TEARDOWN,
                duration_secs=duration,
                result_payload=result,
            )
        )
        await self._job_queue.ack(step.step_id, result_json=result.model_dump_json())

        # Determine final outcome from the execute step
        steps = await self._state_store.get_steps_for_dispatch(dispatch.workflow_id)
        execute_step = next(
            (s for s in steps if s.step_type == StepType.EXECUTE),
            None,
        )
        if execute_step and execute_step.result_json:
            exec_result = ExecuteResult.model_validate_json(execute_step.result_json)
            final_outcome = exec_result.outcome
        elif execute_step and execute_step.status == StepStatus.FAILED:
            final_outcome = Outcome.ERROR
        else:
            final_outcome = Outcome.SUCCESS

        # Calculate total dispatch duration
        dispatch_view = await self._state_store.get_dispatch(dispatch.workflow_id)
        total_duration = duration  # fallback
        if dispatch_view:
            try:
                created = datetime.fromisoformat(dispatch_view.created_at)
                total_duration = int((datetime.now(UTC) - created).total_seconds())
            except ValueError, TypeError:
                pass

        # Use FAILED status for non-success outcomes
        now_ts = datetime.now(UTC).isoformat()
        if final_outcome in (Outcome.ERROR, Outcome.TIMEOUT):
            await self._event_store.append(
                DispatchFailed(
                    timestamp=now_ts,
                    workflow_id=dispatch.workflow_id,
                    failed_step_id=step.step_id,
                    failed_step_type=StepType.EXECUTE,
                    error=f"Execution outcome: {final_outcome}",
                )
            )
            await self._state_store.update_dispatch_status(
                dispatch.workflow_id,
                DispatchStatus.FAILED,
                final_outcome,
            )
        else:
            await self._event_store.append(
                DispatchCompleted(
                    timestamp=now_ts,
                    workflow_id=dispatch.workflow_id,
                    outcome=final_outcome,
                    total_duration_secs=total_duration,
                )
            )
            await self._state_store.update_dispatch_status(
                dispatch.workflow_id,
                DispatchStatus.COMPLETED,
                final_outcome,
            )

    # ── Dry run ───────────────────────────────────────────────────────────

    async def _do_dry_run(
        self,
        payload: DryRunStepPayload,
        step: QueuedStep,
    ) -> None:
        """Execute a dry-run step — check what provisioning would do."""
        dispatch = payload.dispatch

        # Build VMRequirements from dispatch's resolved_profile
        from tanren_core.adapters.remote_types import VMRequirements

        profile = dispatch.resolved_profile
        requirements = VMRequirements(
            profile=dispatch.environment_profile,
            cpu=profile.resources.cpu if profile else 2,
            memory_gb=profile.resources.memory_gb if profile else 4,
            gpu=profile.resources.gpu if profile else False,
            server_type=profile.server_type if profile else None,
        )

        info = await self._execution_env.dry_run(requirements)

        result = DryRunResult(
            provider=str(info.provider),
            server_type=info.server_type,
            estimated_cost_hourly=info.estimated_cost_hourly,
            would_provision=info.would_provision,
        )

        await self._event_store.append(
            StepCompleted(
                timestamp=datetime.now(UTC).isoformat(),
                workflow_id=dispatch.workflow_id,
                step_id=step.step_id,
                step_type=StepType.DRY_RUN,
                duration_secs=0,
                result_payload=result,
            )
        )
        await self._job_queue.ack(step.step_id, result_json=result.model_dump_json())

        # DRY_RUN is a terminal step — mark dispatch completed
        await self._state_store.update_dispatch_status(
            dispatch.workflow_id,
            DispatchStatus.COMPLETED,
        )

    # ── Error handling ────────────────────────────────────────────────────

    async def _handle_step_failure(
        self,
        step: QueuedStep,
        exc: Exception,
    ) -> None:
        """Handle a step failure — retry or mark terminal."""
        now = datetime.now(UTC).isoformat()
        error_msg = str(exc)

        # Classify error for retry decision
        error_class = ErrorClass.FATAL
        try:
            error_class = classify_error(
                exit_code=-1,
                stdout="",
                stderr=error_msg,
                signal_value=None,
            )
        except Exception:
            pass

        # Get current retry count from step view
        step_view = await self._state_store.get_step(step.step_id)
        retry_count = step_view.retry_count if step_view else 0

        should_retry = error_class == ErrorClass.TRANSIENT and retry_count < _MAX_RETRIES

        await self._event_store.append(
            StepFailed(
                timestamp=now,
                workflow_id=step.dispatch_id,
                step_id=step.step_id,
                step_type=step.step_type,
                error=error_msg,
                error_class=str(error_class.value),
                retry_count=retry_count + 1,
                duration_secs=0,
            )
        )
        await self._event_store.append(
            ErrorOccurred(
                timestamp=now,
                workflow_id=step.dispatch_id,
                phase=str(step.step_type),
                error=error_msg,
                error_class=str(error_class.value),
            )
        )

        if should_retry:
            logger.warning(
                "Retrying step %s (attempt %d/%d): %s",
                step.step_id,
                retry_count + 1,
                _MAX_RETRIES,
                error_msg,
            )
            await self._job_queue.nack(
                step.step_id,
                error=error_msg,
                error_class=str(error_class.value),
                retry=True,
            )
        else:
            logger.error(
                "Step %s failed terminally: %s",
                step.step_id,
                error_msg,
            )
            await self._job_queue.nack(
                step.step_id,
                error=error_msg,
                error_class=str(error_class.value),
                retry=False,
            )
            await self._mark_dispatch_failed(
                step.dispatch_id,
                step.step_id,
                step.step_type,
                error_msg,
            )

    # ── Helpers ───────────────────────────────────────────────────────────

    @staticmethod
    def _build_next_step(
        *,
        dispatch: Dispatch,
        next_type: StepType,
        next_sequence: int,
        handle: PersistedEnvironmentHandle,
        preserve: bool = False,
    ) -> _NextStepKwargs | None:
        """Build kwargs for ``ack_and_enqueue()`` without calling the queue.

        Returns:
            A ``_NextStepKwargs`` dict with the ``next_*`` keyword arguments,
            or ``None`` if the dispatch is not a valid ``Dispatch`` instance.
        """
        from tanren_core.schemas import Dispatch

        disp = dispatch if isinstance(dispatch, Dispatch) else None
        if disp is None:
            return None

        step_id = uuid.uuid4().hex
        lane: Lane | None = None
        if next_type == StepType.EXECUTE:
            from tanren_core.store.enums import cli_to_lane

            lane = cli_to_lane(disp.cli)
            step_payload = ExecuteStepPayload(dispatch=disp, handle=handle)
        elif next_type == StepType.TEARDOWN:
            step_payload = TeardownStepPayload(dispatch=disp, handle=handle, preserve=preserve)
        else:
            return None

        return _NextStepKwargs(
            next_step_id=step_id,
            next_dispatch_id=disp.workflow_id,
            next_step_type=str(next_type),
            next_step_sequence=next_sequence,
            next_lane=str(lane) if lane else None,
            next_payload_json=step_payload.model_dump_json(),
        )

    async def _mark_dispatch_failed(
        self,
        dispatch_id: str,
        failed_step_id: str,
        failed_step_type: StepType,
        error: str,
    ) -> None:
        """Mark a dispatch as terminally failed."""
        await self._event_store.append(
            DispatchFailed(
                timestamp=datetime.now(UTC).isoformat(),
                workflow_id=dispatch_id,
                failed_step_id=failed_step_id,
                failed_step_type=failed_step_type,
                error=error,
            )
        )
        await self._state_store.update_dispatch_status(
            dispatch_id,
            DispatchStatus.FAILED,
            Outcome.ERROR,
        )

    @staticmethod
    def _persist_handle(
        handle: EnvironmentHandle,
        *,
        dispatch_id: str | None = None,
    ) -> PersistedEnvironmentHandle:
        """Convert a live EnvironmentHandle to a serializable form."""
        from tanren_core.adapters.types import RemoteEnvironmentRuntime

        vm = None
        ssh_config = None
        workspace_remote_path = None
        agent_user = None
        teardown_commands: tuple[str, ...] = ()
        task_env: dict[str, str] = {}
        profile_name = "default"

        if handle.runtime.kind == "remote":
            rt = handle.runtime
            if isinstance(rt, RemoteEnvironmentRuntime):
                from tanren_core.store.handle import PersistedSSHConfig, PersistedVMInfo

                vm = PersistedVMInfo(
                    vm_id=rt.vm_handle.vm_id,
                    host=rt.vm_handle.host,
                    provider=rt.vm_handle.provider,
                    created_at=rt.vm_handle.created_at,
                    labels=dict(rt.vm_handle.labels),
                    hourly_cost=rt.vm_handle.hourly_cost,
                )
                # Extract SSH config from live connection
                from tanren_core.adapters.ssh import SSHConnection as _SSHConn

                if isinstance(rt.connection, _SSHConn):
                    conn_cfg = rt.connection._config
                    ssh_config = PersistedSSHConfig(
                        host=conn_cfg.host,
                        user=conn_cfg.user,
                        key_path=conn_cfg.key_path,
                        port=conn_cfg.port,
                        connect_timeout=conn_cfg.connect_timeout,
                        host_key_policy=conn_cfg.host_key_policy,
                    )
                else:
                    ssh_config = PersistedSSHConfig(host=rt.vm_handle.host)
                workspace_remote_path = rt.workspace_path.path
                teardown_commands = rt.teardown_commands
                profile_name = rt.profile.name
        elif handle.runtime.kind == "local":
            from tanren_core.adapters.types import LocalEnvironmentRuntime

            if isinstance(handle.runtime, LocalEnvironmentRuntime):
                task_env = dict(handle.runtime.task_env)

        return PersistedEnvironmentHandle(
            env_id=handle.env_id,
            worktree_path=str(handle.worktree_path),
            branch=handle.branch,
            project=handle.project,
            vm=vm,
            ssh_config=ssh_config,
            workspace_remote_path=workspace_remote_path,
            teardown_commands=teardown_commands,
            profile_name=profile_name,
            dispatch_id=dispatch_id,
            provision_timestamp=datetime.now(UTC).isoformat(),
            agent_user=agent_user,
            task_env=task_env,
        )

    @staticmethod
    def _reconstruct_handle(
        persisted: PersistedEnvironmentHandle,
    ) -> EnvironmentHandle:
        """Reconstruct a live EnvironmentHandle from persisted data."""
        from pathlib import Path

        from tanren_core.adapters.types import (
            LocalEnvironmentRuntime,
            RemoteEnvironmentRuntime,
        )

        if persisted.vm is not None and persisted.ssh_config is not None:
            # Remote handle — create fresh SSH connection
            from tanren_core.adapters.remote_types import VMHandle, WorkspacePath
            from tanren_core.adapters.ssh import SSHConfig, SSHConnection
            from tanren_core.env.environment_schema import EnvironmentProfile

            vm_handle = VMHandle(
                vm_id=persisted.vm.vm_id,
                host=persisted.vm.host,
                provider=persisted.vm.provider,
                created_at=persisted.vm.created_at,
                labels=dict(persisted.vm.labels),
                hourly_cost=persisted.vm.hourly_cost,
            )
            ssh_cfg = SSHConfig(
                host=persisted.ssh_config.host,
                user=persisted.ssh_config.user,
                key_path=persisted.ssh_config.key_path,
                port=persisted.ssh_config.port,
                connect_timeout=persisted.ssh_config.connect_timeout,
                host_key_policy=persisted.ssh_config.host_key_policy,
            )
            conn = SSHConnection(config=ssh_cfg)
            workspace = WorkspacePath(
                path=persisted.workspace_remote_path or persisted.worktree_path,
                project=persisted.project,
                branch=persisted.branch,
            )
            # Compute provision_start from persisted timestamp so
            # VM duration calculations reflect real elapsed time
            provision_start = time.monotonic()
            try:
                prov_dt = datetime.fromisoformat(persisted.provision_timestamp)
                elapsed = (datetime.now(UTC) - prov_dt).total_seconds()
                provision_start = time.monotonic() - elapsed
            except ValueError, TypeError:
                pass

            runtime = RemoteEnvironmentRuntime(
                vm_handle=vm_handle,
                connection=conn,
                workspace_path=workspace,
                profile=EnvironmentProfile(name=persisted.profile_name),
                teardown_commands=persisted.teardown_commands,
                provision_start=provision_start,
                workflow_id=persisted.dispatch_id or f"reconstructed-{persisted.env_id}",
            )
        else:
            # Local handle
            runtime = LocalEnvironmentRuntime(
                task_env=dict(persisted.task_env),
            )

        return EnvironmentHandle(
            env_id=persisted.env_id,
            worktree_path=Path(persisted.worktree_path),
            branch=persisted.branch,
            project=persisted.project,
            runtime=runtime,
        )
