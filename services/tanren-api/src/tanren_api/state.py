"""In-memory API state store for tracking in-flight dispatches and environments."""
# ruff: noqa: DOC201

from __future__ import annotations

import asyncio
import contextlib
import logging
from dataclasses import dataclass, replace
from datetime import UTC, datetime, timedelta
from pathlib import Path

from tanren_api.models import DispatchRunStatus, RunEnvironmentStatus
from tanren_core.adapters.types import EnvironmentHandle
from tanren_core.schemas import Dispatch, Outcome, Phase


class _UnsetType:
    """Sentinel for 'no change' in update/transition methods."""

    _instance: _UnsetType | None = None

    def __new__(cls) -> _UnsetType:
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __repr__(self) -> str:
        return "_UNSET"


_UNSET = _UnsetType()

logger = logging.getLogger(__name__)

_SHUTDOWN_TIMEOUT_SECS = 10
_MAX_TERMINAL_DISPATCH_AGE_SECS = 3600
_TERMINAL_STATUSES = frozenset({
    DispatchRunStatus.COMPLETED,
    DispatchRunStatus.FAILED,
    DispatchRunStatus.CANCELLED,
})


@dataclass
class DispatchRecord:
    """Tracks an in-flight dispatch."""

    dispatch_id: str
    dispatch: Dispatch
    status: DispatchRunStatus
    outcome: Outcome | None = None
    created_at: str = ""
    started_at: str | None = None
    completed_at: str | None = None
    task: asyncio.Task[None] | None = None
    dispatch_path: Path | None = None


@dataclass
class EnvironmentRecord:
    """Tracks a provisioned execution environment."""

    env_id: str
    handle: EnvironmentHandle
    status: RunEnvironmentStatus
    phase: Phase | None = None
    outcome: Outcome | None = None
    vm_id: str = ""
    host: str = ""
    dispatch_id: str | None = None
    started_at: str | None = None
    completed_at: str | None = None
    task: asyncio.Task[None] | None = None


class APIStateStore:
    """In-memory store for dispatches and environments.

    Dict operations are synchronized via asyncio.Lock. Field updates
    should use the update_* methods. Acceptable for single-worker mode.
    """

    def __init__(self) -> None:
        """Initialize empty state."""
        self._dispatches: dict[str, DispatchRecord] = {}
        self._environments: dict[str, EnvironmentRecord] = {}
        self._lock = asyncio.Lock()

    # -- Dispatch operations --

    async def add_dispatch(self, record: DispatchRecord) -> None:
        """Register a new dispatch."""
        async with self._lock:
            self._reap_terminal_dispatches()
            self._dispatches[record.dispatch_id] = record

    def _reap_terminal_dispatches(self) -> None:
        """Remove terminal dispatches older than retention window. Must hold _lock."""
        cutoff = datetime.now(UTC) - timedelta(seconds=_MAX_TERMINAL_DISPATCH_AGE_SECS)
        to_remove = []
        for dispatch_id, record in self._dispatches.items():
            if record.status not in _TERMINAL_STATUSES:
                continue
            if record.completed_at is None:
                continue
            try:
                completed = datetime.fromisoformat(record.completed_at)
                if completed < cutoff:
                    to_remove.append(dispatch_id)
            except ValueError, TypeError:
                continue
        for dispatch_id in to_remove:
            del self._dispatches[dispatch_id]

    async def get_dispatch(self, dispatch_id: str) -> DispatchRecord | None:
        """Look up a dispatch by ID. Returns a defensive copy.

        The returned record is a shallow copy with a deep-copied ``dispatch``
        model, so callers can inspect or mutate it without affecting store state.
        """
        async with self._lock:
            record = self._dispatches.get(dispatch_id)
            if record is None:
                return None
            return replace(record, dispatch=record.dispatch.model_copy(deep=True))

    async def update_dispatch(
        self,
        dispatch_id: str,
        *,
        status: DispatchRunStatus | _UnsetType = _UNSET,
        outcome: Outcome | _UnsetType | None = _UNSET,
        started_at: str | _UnsetType | None = _UNSET,
        completed_at: str | _UnsetType | None = _UNSET,
        task: asyncio.Task[None] | _UnsetType | None = _UNSET,
    ) -> None:
        """Update fields on an existing dispatch record."""
        async with self._lock:
            record = self._dispatches.get(dispatch_id)
            if record is None:
                return
            if not isinstance(status, _UnsetType):
                record.status = status
            if not isinstance(outcome, _UnsetType):
                record.outcome = outcome
            if not isinstance(started_at, _UnsetType):
                record.started_at = started_at
            if not isinstance(completed_at, _UnsetType):
                record.completed_at = completed_at
            if not isinstance(task, _UnsetType):
                record.task = task

    async def try_transition_dispatch(
        self,
        dispatch_id: str,
        *,
        from_statuses: frozenset[DispatchRunStatus],
        to_status: DispatchRunStatus,
        outcome: Outcome | _UnsetType | None = _UNSET,
        started_at: str | _UnsetType | None = _UNSET,
        completed_at: str | _UnsetType | None = _UNSET,
        task: asyncio.Task[None] | _UnsetType | None = _UNSET,
    ) -> DispatchRecord | None:
        """Atomically transition if current status is in *from_statuses*.

        Returns copy of updated record on success, None on mismatch/not-found.
        """
        async with self._lock:
            record = self._dispatches.get(dispatch_id)
            if record is None or record.status not in from_statuses:
                return None
            record.status = to_status
            if not isinstance(outcome, _UnsetType):
                record.outcome = outcome
            if not isinstance(started_at, _UnsetType):
                record.started_at = started_at
            if not isinstance(completed_at, _UnsetType):
                record.completed_at = completed_at
            if not isinstance(task, _UnsetType):
                record.task = task
            return replace(record)

    async def remove_dispatch(self, dispatch_id: str) -> DispatchRecord | None:
        """Remove and return a dispatch record (shallow copy, task is shared)."""
        async with self._lock:
            record = self._dispatches.pop(dispatch_id, None)
            return replace(record) if record is not None else None

    # -- Environment operations --

    async def add_environment(self, record: EnvironmentRecord) -> None:
        """Register a new environment."""
        async with self._lock:
            self._environments[record.env_id] = record

    async def get_environment(self, env_id: str) -> EnvironmentRecord | None:
        """Look up an environment by ID. Returns a defensive copy.

        Scalar fields (status, phase, outcome, etc.) are independent of the
        stored record.  ``handle`` and ``task`` are shared references: handle
        contains a live SSH connection that cannot be safely deep-copied, and
        task is intentionally shared for cancellation.
        """
        async with self._lock:
            record = self._environments.get(env_id)
            return replace(record) if record is not None else None

    async def update_environment(
        self,
        env_id: str,
        *,
        status: RunEnvironmentStatus | _UnsetType = _UNSET,
        phase: Phase | _UnsetType | None = _UNSET,
        outcome: Outcome | _UnsetType | None = _UNSET,
        dispatch_id: str | _UnsetType | None = _UNSET,
        started_at: str | _UnsetType | None = _UNSET,
        completed_at: str | _UnsetType | None = _UNSET,
        task: asyncio.Task[None] | _UnsetType | None = _UNSET,
    ) -> None:
        """Update fields on an existing environment record."""
        async with self._lock:
            record = self._environments.get(env_id)
            if record is None:
                return
            if not isinstance(status, _UnsetType):
                record.status = status
            if not isinstance(phase, _UnsetType):
                record.phase = phase
            if not isinstance(outcome, _UnsetType):
                record.outcome = outcome
            if not isinstance(dispatch_id, _UnsetType):
                record.dispatch_id = dispatch_id
            if not isinstance(started_at, _UnsetType):
                record.started_at = started_at
            if not isinstance(completed_at, _UnsetType):
                record.completed_at = completed_at
            if not isinstance(task, _UnsetType):
                record.task = task

    async def cancel_environment_task(self, env_id: str, *, wait_secs: float = 5.0) -> bool:
        """Cancel a running task and wait for it to stop.

        Returns True if no task is running (none attached, already finished,
        or stopped within *wait_secs*). Returns False only if the task is
        still running after the timeout.
        """
        task: asyncio.Task[None] | None = None
        async with self._lock:
            record = self._environments.get(env_id)
            if record is None or record.task is None or record.task.done():
                return True  # Nothing running — safe to proceed
            record.task.cancel()
            task = record.task
        # Await outside lock so task can acquire lock for status updates
        if task is not None:
            with contextlib.suppress(TimeoutError, asyncio.CancelledError, Exception):
                await asyncio.wait_for(asyncio.shield(task), timeout=wait_secs)
            return task.done()
        return True  # Defensive — shouldn't reach here

    async def remove_environment(self, env_id: str) -> EnvironmentRecord | None:
        """Remove and return an environment record (shallow copy; handle/task shared)."""
        async with self._lock:
            record = self._environments.pop(env_id, None)
            return replace(record) if record is not None else None

    async def try_transition_environment(
        self,
        env_id: str,
        *,
        from_statuses: frozenset[RunEnvironmentStatus],
        to_status: RunEnvironmentStatus,
        phase: Phase | _UnsetType | None = _UNSET,
        outcome: Outcome | _UnsetType | None = _UNSET,
        dispatch_id: str | _UnsetType | None = _UNSET,
        started_at: str | _UnsetType | None = _UNSET,
        completed_at: str | _UnsetType | None = _UNSET,
        task: asyncio.Task[None] | _UnsetType | None = _UNSET,
    ) -> EnvironmentRecord | None:
        """Atomically transition if current status is in *from_statuses*.

        Returns copy of updated record on success, None on mismatch/not-found.
        """
        async with self._lock:
            record = self._environments.get(env_id)
            if record is None or record.status not in from_statuses:
                return None
            record.status = to_status
            if not isinstance(phase, _UnsetType):
                record.phase = phase
            if not isinstance(outcome, _UnsetType):
                record.outcome = outcome
            if not isinstance(dispatch_id, _UnsetType):
                record.dispatch_id = dispatch_id
            if not isinstance(started_at, _UnsetType):
                record.started_at = started_at
            if not isinstance(completed_at, _UnsetType):
                record.completed_at = completed_at
            if not isinstance(task, _UnsetType):
                record.task = task
            return replace(record)

    # -- Shutdown --

    async def shutdown(self) -> None:
        """Cancel all running tasks and await with timeout."""
        tasks: list[asyncio.Task[None]] = []
        async with self._lock:
            for record in self._dispatches.values():
                if record.task is not None and not record.task.done():
                    record.task.cancel()
                    tasks.append(record.task)
            for record in self._environments.values():
                if record.task is not None and not record.task.done():
                    record.task.cancel()
                    tasks.append(record.task)

        if tasks:
            logger.info("Shutting down %d background tasks", len(tasks))
            _done, pending = await asyncio.wait(tasks, timeout=_SHUTDOWN_TIMEOUT_SECS)
            if pending:
                logger.warning("%d tasks did not complete within shutdown timeout", len(pending))
