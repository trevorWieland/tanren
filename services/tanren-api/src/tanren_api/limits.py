"""Resource limit enforcement for scoped API keys."""

from __future__ import annotations

from datetime import UTC, datetime, timedelta

from tanren_api.errors import ForbiddenError
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.auth_views import AuthContext


def _fmt(dt: datetime) -> str:
    """Format a datetime as ISO 8601 with consistent microsecond precision."""
    # Always include 6-digit microseconds for safe lexicographic TEXT comparison in the DB
    return dt.strftime("%Y-%m-%dT%H:%M:%S.%fZ")


def _one_hour_ago() -> str:
    return _fmt(datetime.now(UTC) - timedelta(hours=1))


def _start_of_day() -> str:
    now = datetime.now(UTC)
    return _fmt(now.replace(hour=0, minute=0, second=0, microsecond=0))


async def check_resource_limits(
    auth: AuthContext,
    auth_store: AuthStore,
    action: str,
) -> None:
    """Raise ForbiddenError if any resource limit would be exceeded.

    Args:
        auth: Resolved auth context for the request.
        auth_store: Store for querying current usage.
        action: ``"dispatch"`` or ``"vm_provision"``.

    Raises:
        ForbiddenError: With an explanation of which limit was exceeded.
    """
    rl = auth.resource_limits
    if rl is None:
        return

    user_id = auth.user.user_id

    # Dispatch rate limit (sliding 60-minute window)
    if action == "dispatch" and rl.max_dispatches_per_hour is not None:
        since = _one_hour_ago()
        count = await auth_store.count_dispatches_since(user_id, since)
        if count >= rl.max_dispatches_per_hour:
            raise ForbiddenError(
                f"Rate limit exceeded: {count}/{rl.max_dispatches_per_hour} dispatches per hour"
            )

    # Concurrent VM limit
    if action in ("vm_provision", "dispatch") and rl.max_concurrent_vms is not None:
        active = await auth_store.count_active_vms(user_id)
        if active >= rl.max_concurrent_vms:
            raise ForbiddenError(
                f"VM limit exceeded: {active}/{rl.max_concurrent_vms} concurrent VMs"
            )

    # Daily cost ceiling
    if rl.max_cost_per_day is not None:
        since = _start_of_day()
        cost = await auth_store.sum_cost_since(user_id, since)
        if cost >= rl.max_cost_per_day:
            raise ForbiddenError(
                f"Cost limit exceeded: ${cost:.2f}/${rl.max_cost_per_day:.2f} per day"
            )
