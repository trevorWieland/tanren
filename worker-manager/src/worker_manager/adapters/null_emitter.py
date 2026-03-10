"""No-op event emitter — default when no events DB is configured."""

from __future__ import annotations

from worker_manager.adapters.events import Event


class NullEventEmitter:
    """Silently discards all events."""

    async def emit(self, event: Event) -> None:
        pass

    async def close(self) -> None:
        pass
