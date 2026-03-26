"""Resource limit enforcement for scoped API keys."""

from __future__ import annotations

from datetime import UTC, datetime, timedelta

from tanren_api.errors import ForbiddenError
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.auth_views import AuthContext


def _now_iso() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def _one_hour_ago() -> str:
    return (datetime.now(UTC) - timedelta(hours=1)).isoformat().replace("+00:00", "Z")


def _start_of_day() -> str:
    now = datetime.now(UTC)
    return now.replace(hour=0, minute=0, second=0, microsecond=0).isoformat().replace("+00:00", "Z")


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
