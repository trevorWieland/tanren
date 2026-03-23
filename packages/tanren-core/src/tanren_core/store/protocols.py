"""Core protocols for the event-sourced store layer.

Three protocols define the contract between business logic and storage:

- ``EventStore`` — append-only event log with projection maintenance
- ``JobQueue`` — step-based job queue backed by the step_projection table
- ``StateStore`` — read-only queries against projection tables
"""

from __future__ import annotations

from typing import Protocol, runtime_checkable

from tanren_core.adapters.events import Event
from tanren_core.schemas import Outcome
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane
from tanren_core.store.views import (
    DispatchListFilter,
    DispatchView,
    EventQueryResult,
    QueuedStep,
    StepView,
)


@runtime_checkable
class EventStore(Protocol):
    """Append-only event store with transactional projection maintenance.

    On each ``append()``, the implementation MUST transactionally:

    1. INSERT the event into the ``events`` table.
    2. UPDATE the ``dispatch_projection`` row (status, outcome, updated_at).
    3. UPDATE the ``step_projection`` row (status, worker_id, result_json).

    This ensures projections are always consistent with the event log.
    """

    async def append(self, event: Event) -> None:
        """Append an event and update projections transactionally.

        For SQLite: wraps in ``BEGIN IMMEDIATE``.
        For Postgres: wraps in a transaction.
        """
        ...

    async def query_events(
        self,
        *,
        dispatch_id: str | None = None,
        event_type: str | None = None,
        since: str | None = None,
        until: str | None = None,
        limit: int = 50,
        offset: int = 0,
    ) -> EventQueryResult:
        """Query events with optional filters and pagination."""
        ...

    async def close(self) -> None:
        """Close any resources held by the store."""
        ...


@runtime_checkable
class JobQueue(Protocol):
    """Step-based job queue backed by the ``step_projection`` table.

    ``dequeue()`` atomically claims a step matching the lane constraint.
    ``ack()`` marks it completed.  ``nack()`` marks it failed or
    re-enqueues for retry.
    """

    async def enqueue_step(
        self,
        *,
        step_id: str,
        dispatch_id: str,
        step_type: str,
        step_sequence: int,
        lane: str | None,
        payload_json: str,
    ) -> None:
        """Insert a new step into the queue.

        Creates a ``step_projection`` row with ``status='pending'`` and
        appends a ``StepEnqueued`` event, all in one transaction.
        """
        ...

    async def dequeue(
        self,
        *,
        lane: Lane | None = None,
        worker_id: str,
        max_concurrent: int,
    ) -> QueuedStep | None:
        """Atomically claim a pending step if running_count < max_concurrent.

        For SQLite: ``BEGIN IMMEDIATE; SELECT ... WHERE status='pending'
        AND lane IS ? AND running_count < ?; UPDATE SET status='running';
        COMMIT;``

        For Postgres: ``SELECT ... FOR UPDATE SKIP LOCKED`` with a
        running-count subquery.

        Returns ``None`` if no work is available or capacity is full.
        """
        ...

    async def ack(self, step_id: str, *, result_json: str) -> None:
        """Mark a step as completed and store its result.

        Updates ``step_projection.status = 'completed'`` and
        ``result_json`` in one transaction.
        """
        ...

    async def ack_and_enqueue(
        self,
        step_id: str,
        *,
        result_json: str,
        next_step_id: str,
        next_dispatch_id: str,
        next_step_type: str,
        next_step_sequence: int,
        next_lane: str | None,
        next_payload_json: str,
        completion_events: list[Event] | None = None,
    ) -> None:
        """Atomically acknowledge a step and enqueue the next step.

        Combines the ack (UPDATE step to completed) and enqueue (INSERT
        new step + INSERT StepEnqueued event + UPDATE dispatch status) in
        a single transaction to prevent race conditions during auto-chaining.
        """
        ...

    async def cancel_pending_steps(self, dispatch_id: str) -> int:
        """Cancel all pending steps for a dispatch.

        Sets ``status='cancelled'`` on every ``step_projection`` row
        belonging to *dispatch_id* that is still ``'pending'``.

        Returns the number of rows updated.
        """
        ...

    async def nack(
        self,
        step_id: str,
        *,
        error: str,
        error_class: str | None = None,
        retry: bool = False,
    ) -> None:
        """Mark a step as failed.

        If ``retry=True``, increments ``retry_count`` and resets
        ``status`` to ``'pending'``.  Otherwise sets ``status='failed'``.
        """
        ...

    async def close(self) -> None:
        """Close any resources."""
        ...


@runtime_checkable
class StateStore(Protocol):
    """State queries and mutations against projection tables."""

    async def get_dispatch(self, dispatch_id: str) -> DispatchView | None:
        """Look up a dispatch by ID from the ``dispatch_projection`` table."""
        ...

    async def query_dispatches(self, filters: DispatchListFilter) -> list[DispatchView]:
        """Query dispatches with filters and pagination."""
        ...

    async def get_step(self, step_id: str) -> StepView | None:
        """Look up a step by ID from the ``step_projection`` table."""
        ...

    async def get_steps_for_dispatch(self, dispatch_id: str) -> list[StepView]:
        """Get all steps for a dispatch, ordered by ``step_sequence``."""
        ...

    async def count_running_steps(self, *, lane: Lane | None = None) -> int:
        """Count steps with ``status='running'`` for the given lane."""
        ...

    async def create_dispatch_projection(
        self,
        *,
        dispatch_id: str,
        mode: DispatchMode,
        lane: Lane,
        preserve_on_failure: bool,
        dispatch_json: str,
    ) -> None:
        """Insert a new dispatch projection row."""
        ...

    async def update_dispatch_status(
        self,
        dispatch_id: str,
        status: DispatchStatus,
        outcome: Outcome | None = None,
    ) -> None:
        """Update dispatch projection status and outcome."""
        ...

    async def close(self) -> None:
        """Close any resources."""
        ...
