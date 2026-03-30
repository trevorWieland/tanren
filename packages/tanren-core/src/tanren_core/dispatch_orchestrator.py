"""Shared dispatch orchestration — single source of truth for all entry points.

This module owns the three core store operations that every dispatch lifecycle
path (CLI, MCP, REST API) must perform:

1. Create dispatch projection
2. Append ``DispatchCreated`` event
3. Enqueue the first step

It also centralises state guards (concurrent-execute, post-teardown, etc.)
so that CLI, API services, and MCP tools share one implementation.

The module accepts ``tanren_core`` types only — never API request models —
so it stays importable from every package without circular dependencies.
"""

from __future__ import annotations

import uuid
from dataclasses import dataclass
from datetime import UTC, datetime

from tanren_core.schemas import Dispatch
from tanren_core.store.enums import (
    DispatchMode,
    StepStatus,
    StepType,
    cli_to_lane,
)
from tanren_core.store.events import DispatchCreated
from tanren_core.store.handle import PersistedEnvironmentHandle
from tanren_core.store.payloads import (
    DryRunStepPayload,
    ExecuteStepPayload,
    ProvisionResult,
    ProvisionStepPayload,
    TeardownStepPayload,
)
from tanren_core.store.protocols import EventStore, JobQueue, StateStore
from tanren_core.store.views import StepView

# ---------------------------------------------------------------------------
# Result types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class DispatchResult:
    """Returned by :func:`create_dispatch` and :func:`enqueue_dry_run_step`."""

    dispatch_id: str
    dispatch: Dispatch
    step_id: str


@dataclass(frozen=True)
class StepEnqueueResult:
    """Returned by :func:`enqueue_execute_step` and :func:`enqueue_teardown_step`."""

    step_id: str
    dispatch_id: str
    step_sequence: int


# ---------------------------------------------------------------------------
# Guard errors (core-level — callers map to HTTP / CLI errors)
# ---------------------------------------------------------------------------


class DispatchGuardError(Exception):
    """Base error raised when a lifecycle state guard blocks an operation."""


class ConcurrentExecuteError(DispatchGuardError):
    """An execute step is already pending or running."""


class PostTeardownExecuteError(DispatchGuardError):
    """Cannot execute after teardown has been requested."""


class ActiveExecuteTeardownError(DispatchGuardError):
    """Cannot teardown while an execute step is active."""


class DuplicateTeardownError(DispatchGuardError):
    """Teardown is already enqueued, running, or completed."""


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------


def _now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def _next_sequence(steps: list[StepView]) -> int:
    return max((s.step_sequence for s in steps), default=0) + 1


# ---------------------------------------------------------------------------
# Guard functions
# ---------------------------------------------------------------------------


async def check_execute_guards(state_store: StateStore, dispatch_id: str) -> None:
    """Verify that an execute step is allowed for *dispatch_id*.

    May raise ``ConcurrentExecuteError`` or ``PostTeardownExecuteError``.
    """
    steps = await state_store.get_steps_for_dispatch(dispatch_id)
    _check_execute_guards_from_steps(steps)


def _check_execute_guards_from_steps(steps: list[StepView]) -> None:
    if any(
        s.step_type == StepType.EXECUTE and s.status in (StepStatus.PENDING, StepStatus.RUNNING)
        for s in steps
    ):
        raise ConcurrentExecuteError("Execute step already in progress")

    if any(
        s.step_type == StepType.TEARDOWN
        and s.status in (StepStatus.PENDING, StepStatus.RUNNING, StepStatus.COMPLETED)
        for s in steps
    ):
        raise PostTeardownExecuteError("Cannot execute after teardown")


async def check_teardown_guards(
    state_store: StateStore,
    dispatch_id: str,
    *,
    allow_retry_after_failure: bool = False,
) -> None:
    """Verify that a teardown step is allowed for *dispatch_id*.

    Args:
        state_store: Projection read/write.
        dispatch_id: The parent dispatch ID.
        allow_retry_after_failure: When ``True``, a previously-failed teardown
            does not block a new attempt. Used by ``VMService.release()``.

    May raise ``ActiveExecuteTeardownError`` or ``DuplicateTeardownError``.
    """
    steps = await state_store.get_steps_for_dispatch(dispatch_id)
    _check_teardown_guards_from_steps(steps, allow_retry_after_failure=allow_retry_after_failure)


def _check_teardown_guards_from_steps(
    steps: list[StepView],
    *,
    allow_retry_after_failure: bool = False,
) -> None:
    if any(
        s.step_type == StepType.EXECUTE and s.status in (StepStatus.PENDING, StepStatus.RUNNING)
        for s in steps
    ):
        raise ActiveExecuteTeardownError("Cannot teardown while execute is in progress")

    blocked_statuses = {StepStatus.PENDING, StepStatus.RUNNING, StepStatus.COMPLETED}
    if not allow_retry_after_failure:
        # Default: also block if a non-failed teardown exists
        if any(s.step_type == StepType.TEARDOWN and s.status in blocked_statuses for s in steps):
            raise DuplicateTeardownError("Teardown already enqueued or completed")
    else:
        # VM release mode: allow retry when all existing teardowns failed
        if any(s.step_type == StepType.TEARDOWN and s.status != StepStatus.FAILED for s in steps):
            raise DuplicateTeardownError("Teardown already enqueued or completed")


# ---------------------------------------------------------------------------
# Core orchestration functions
# ---------------------------------------------------------------------------


async def create_dispatch(
    *,
    dispatch: Dispatch,
    mode: DispatchMode,
    event_store: EventStore,
    job_queue: JobQueue,
    state_store: StateStore,
    user_id: str = "",
    preserve_on_failure: bool | None = None,
) -> DispatchResult:
    """Create a dispatch projection, append ``DispatchCreated``, enqueue provision step.

    This is the single entry point for dispatch creation across CLI, MCP,
    and REST API.  Callers build the ``Dispatch`` model and generate the
    ``workflow_id`` in their preferred format.

    Args:
        dispatch: Fully-constructed Dispatch model.
        mode: AUTO (worker auto-chains steps) or MANUAL (caller drives steps).
        event_store: Append-only event log.
        job_queue: Step-based job queue.
        state_store: Projection read/write.
        user_id: Owner user ID for attribution.
        preserve_on_failure: Override dispatch.preserve_on_failure if set.

    Returns:
        DispatchResult with dispatch_id, dispatch, and step_id.
    """
    dispatch_id = dispatch.workflow_id
    lane = cli_to_lane(dispatch.cli)
    preserve = (
        preserve_on_failure if preserve_on_failure is not None else dispatch.preserve_on_failure
    )

    await state_store.create_dispatch_projection(
        dispatch_id=dispatch_id,
        mode=mode,
        lane=lane,
        preserve_on_failure=preserve,
        dispatch_json=dispatch.model_dump_json(),
        user_id=user_id,
    )

    await event_store.append(
        DispatchCreated(
            timestamp=_now(),
            entity_id=dispatch_id,
            dispatch=dispatch,
            mode=mode,
            lane=lane,
        )
    )

    step_id = uuid.uuid4().hex
    payload = ProvisionStepPayload(dispatch=dispatch)
    await job_queue.enqueue_step(
        step_id=step_id,
        dispatch_id=dispatch_id,
        step_type="provision",
        step_sequence=0,
        lane=None,
        payload_json=payload.model_dump_json(),
    )

    return DispatchResult(dispatch_id=dispatch_id, dispatch=dispatch, step_id=step_id)


async def enqueue_execute_step(
    *,
    dispatch_id: str,
    exec_dispatch: Dispatch,
    handle: PersistedEnvironmentHandle,
    job_queue: JobQueue,
    state_store: StateStore,
    check_guards: bool = True,
) -> StepEnqueueResult:
    """Enqueue an execute step for a provisioned environment.

    Args:
        dispatch_id: The parent dispatch ID.
        exec_dispatch: Dispatch model for this execute phase (may differ from
            the original provision-time dispatch in phase, cli, timeout, etc.).
        handle: Environment handle from the provision step result.
        job_queue: Step-based job queue.
        state_store: Projection read/write.
        check_guards: If True, verify no concurrent execute and no post-teardown.

    Returns:
        StepEnqueueResult with step_id, dispatch_id, and step_sequence.

    May raise ``ConcurrentExecuteError`` or ``PostTeardownExecuteError``
    when ``check_guards`` is True.
    """
    steps = await state_store.get_steps_for_dispatch(dispatch_id)

    if check_guards:
        _check_execute_guards_from_steps(steps)

    lane = cli_to_lane(exec_dispatch.cli)
    seq = _next_sequence(steps)
    step_id = uuid.uuid4().hex
    payload = ExecuteStepPayload(dispatch=exec_dispatch, handle=handle)

    await job_queue.enqueue_step(
        step_id=step_id,
        dispatch_id=dispatch_id,
        step_type="execute",
        step_sequence=seq,
        lane=str(lane),
        payload_json=payload.model_dump_json(),
    )

    return StepEnqueueResult(step_id=step_id, dispatch_id=dispatch_id, step_sequence=seq)


async def enqueue_teardown_step(
    *,
    dispatch_id: str,
    dispatch: Dispatch,
    handle: PersistedEnvironmentHandle,
    job_queue: JobQueue,
    state_store: StateStore,
    check_guards: bool = True,
    preserve: bool = False,
    allow_retry_after_failure: bool = False,
) -> StepEnqueueResult:
    """Enqueue a teardown step.

    Args:
        dispatch_id: The parent dispatch ID.
        dispatch: Dispatch model (for workflow_id, project reference).
        handle: Environment handle to tear down.
        job_queue: Step-based job queue.
        state_store: Projection read/write.
        check_guards: If True, verify no active execute and no duplicate teardown.
        preserve: If True, skip actual teardown (preserve_on_failure triggered).
        allow_retry_after_failure: When True, allow re-enqueue when all prior
            teardowns failed (used by VM release).

    Returns:
        StepEnqueueResult with step_id, dispatch_id, and step_sequence.

    May raise ``ActiveExecuteTeardownError`` or ``DuplicateTeardownError``
    when ``check_guards`` is True.
    """
    steps = await state_store.get_steps_for_dispatch(dispatch_id)

    if check_guards:
        _check_teardown_guards_from_steps(
            steps, allow_retry_after_failure=allow_retry_after_failure
        )

    seq = _next_sequence(steps)
    step_id = uuid.uuid4().hex
    payload = TeardownStepPayload(dispatch=dispatch, handle=handle, preserve=preserve)

    await job_queue.enqueue_step(
        step_id=step_id,
        dispatch_id=dispatch_id,
        step_type="teardown",
        step_sequence=seq,
        lane=None,
        payload_json=payload.model_dump_json(),
    )

    return StepEnqueueResult(step_id=step_id, dispatch_id=dispatch_id, step_sequence=seq)


async def enqueue_dry_run_step(
    *,
    dispatch: Dispatch,
    mode: DispatchMode,
    event_store: EventStore,
    job_queue: JobQueue,
    state_store: StateStore,
    user_id: str = "",
    preserve_on_failure: bool = False,
) -> DispatchResult:
    """Create a dispatch and enqueue a dry-run step.

    Identical to :func:`create_dispatch` except the step type is ``dry_run``
    and the payload is ``DryRunStepPayload``.

    Returns:
        DispatchResult with dispatch_id, dispatch, and step_id.
    """
    dispatch_id = dispatch.workflow_id
    lane = cli_to_lane(dispatch.cli)

    await state_store.create_dispatch_projection(
        dispatch_id=dispatch_id,
        mode=mode,
        lane=lane,
        preserve_on_failure=preserve_on_failure,
        dispatch_json=dispatch.model_dump_json(),
        user_id=user_id,
    )

    await event_store.append(
        DispatchCreated(
            timestamp=_now(),
            entity_id=dispatch_id,
            dispatch=dispatch,
            mode=mode,
            lane=lane,
        )
    )

    step_id = uuid.uuid4().hex
    payload = DryRunStepPayload(dispatch=dispatch)
    await job_queue.enqueue_step(
        step_id=step_id,
        dispatch_id=dispatch_id,
        step_type="dry_run",
        step_sequence=0,
        lane=None,
        payload_json=payload.model_dump_json(),
    )

    return DispatchResult(dispatch_id=dispatch_id, dispatch=dispatch, step_id=step_id)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


async def get_provision_result(
    state_store: StateStore,
    dispatch_id: str,
) -> ProvisionResult:
    """Extract the completed provision result for a dispatch.

    Returns:
        ProvisionResult from the completed provision step.

    Raises:
        ValueError: If no completed provision step exists.
    """
    steps = await state_store.get_steps_for_dispatch(dispatch_id)
    provision_step = next(
        (
            s
            for s in steps
            if s.step_type == StepType.PROVISION and s.status == StepStatus.COMPLETED
        ),
        None,
    )
    if provision_step is None or provision_step.result_json is None:
        msg = f"No completed provision step for dispatch {dispatch_id}"
        raise ValueError(msg)
    return ProvisionResult.model_validate_json(provision_step.result_json)
