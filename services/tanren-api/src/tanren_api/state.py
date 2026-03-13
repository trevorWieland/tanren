"""In-memory API state store for tracking in-flight dispatches and environments."""
# ruff: noqa: DOC201

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass

from tanren_api.models import DispatchRunStatus, RunEnvironmentStatus
from tanren_core.adapters.types import EnvironmentHandle
from tanren_core.schemas import Dispatch, Outcome, Phase

logger = logging.getLogger(__name__)

_SHUTDOWN_TIMEOUT_SECS = 10


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

    Thread-safe via asyncio.Lock. Acceptable for v1 — API restart
    loses tracking but VMs are recovered by daemon startup recovery.
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
            self._dispatches[record.dispatch_id] = record

    async def get_dispatch(self, dispatch_id: str) -> DispatchRecord | None:
        """Look up a dispatch by ID."""
        async with self._lock:
            return self._dispatches.get(dispatch_id)

    async def update_dispatch(
        self,
        dispatch_id: str,
        *,
        status: DispatchRunStatus | None = None,
        outcome: Outcome | None = None,
        started_at: str | None = None,
        completed_at: str | None = None,
    ) -> None:
        """Update fields on an existing dispatch record."""
        async with self._lock:
            record = self._dispatches.get(dispatch_id)
            if record is None:
                return
            if status is not None:
                record.status = status
            if outcome is not None:
                record.outcome = outcome
            if started_at is not None:
                record.started_at = started_at
            if completed_at is not None:
                record.completed_at = completed_at

    async def remove_dispatch(self, dispatch_id: str) -> DispatchRecord | None:
        """Remove and return a dispatch record."""
        async with self._lock:
            return self._dispatches.pop(dispatch_id, None)

    # -- Environment operations --

    async def add_environment(self, record: EnvironmentRecord) -> None:
        """Register a new environment."""
        async with self._lock:
            self._environments[record.env_id] = record

    async def get_environment(self, env_id: str) -> EnvironmentRecord | None:
        """Look up an environment by ID."""
        async with self._lock:
            return self._environments.get(env_id)

    async def update_environment(
        self,
        env_id: str,
        *,
        status: RunEnvironmentStatus | None = None,
        phase: Phase | None = None,
        outcome: Outcome | None = None,
        dispatch_id: str | None = None,
        started_at: str | None = None,
        completed_at: str | None = None,
        task: asyncio.Task[None] | None = None,
    ) -> None:
        """Update fields on an existing environment record."""
        async with self._lock:
            record = self._environments.get(env_id)
            if record is None:
                return
            if status is not None:
                record.status = status
            if phase is not None:
                record.phase = phase
            if outcome is not None:
                record.outcome = outcome
            if dispatch_id is not None:
                record.dispatch_id = dispatch_id
            if started_at is not None:
                record.started_at = started_at
            if completed_at is not None:
                record.completed_at = completed_at
            if task is not None:
                record.task = task

    async def remove_environment(self, env_id: str) -> EnvironmentRecord | None:
        """Remove and return an environment record."""
        async with self._lock:
            return self._environments.pop(env_id, None)

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
