"""No-op event emitter — default when no events DB is configured."""

from __future__ import annotations

from tanren_core.adapters.events import Event


class NullEventEmitter:
    """Silently discards all events."""

    async def emit(self, event: Event) -> None:
        """Discard the event (no-op)."""

    async def close(self) -> None:
        """No-op close."""
