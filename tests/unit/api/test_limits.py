"""Tests for resource limit enforcement."""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from tanren_api.errors import ForbiddenError
from tanren_api.limits import check_resource_limits
from tanren_core.store.auth_events import ResourceLimits
from tanren_core.store.auth_views import ApiKeyView, AuthContext, UserView


def _make_user(user_id: str = "user-001") -> UserView:
    return UserView(
        user_id=user_id,
        name="Test User",
        email=None,
        role="member",
        is_active=True,
        created_at="2026-01-01T00:00:00Z",
        updated_at="2026-01-01T00:00:00Z",
    )


def _make_key(
    user_id: str = "user-001",
    scopes: list[str] | None = None,
    resource_limits: ResourceLimits | None = None,
) -> ApiKeyView:
    return ApiKeyView(
        key_id="key-001",
        user_id=user_id,
        name="test-key",
        key_prefix="abcd1234",
        key_hash="fakehash",
        scopes=scopes or ["*"],
        resource_limits=resource_limits,
        created_at="2026-01-01T00:00:00Z",
    )


def _make_auth(
    resource_limits: ResourceLimits | None = None,
    user_id: str = "user-001",
) -> AuthContext:
    user = _make_user(user_id)
    key = _make_key(user_id=user_id, resource_limits=resource_limits)
    return AuthContext(
        user=user,
        key=key,
        scopes=frozenset(key.scopes),
        resource_limits=resource_limits,
    )


class TestCheckResourceLimitsNoLimits:
    """When resource_limits is None, nothing is enforced."""

    async def test_no_limits_passes(self) -> None:
        auth = _make_auth(resource_limits=None)
        mock_store = AsyncMock()
        # Should not raise
        await check_resource_limits(auth, mock_store, "dispatch")


class TestCheckResourceLimitsDispatchRate:
    """Test max_dispatches_per_hour enforcement."""

    async def test_under_limit_passes(self) -> None:
        limits = ResourceLimits(max_dispatches_per_hour=5)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_dispatches_since = AsyncMock(return_value=3)

        await check_resource_limits(auth, mock_store, "dispatch")

    async def test_at_limit_raises_forbidden(self) -> None:
        limits = ResourceLimits(max_dispatches_per_hour=5)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_dispatches_since = AsyncMock(return_value=5)

        with pytest.raises(ForbiddenError, match="Rate limit exceeded"):
            await check_resource_limits(auth, mock_store, "dispatch")

    async def test_over_limit_raises_forbidden(self) -> None:
        limits = ResourceLimits(max_dispatches_per_hour=1)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_dispatches_since = AsyncMock(return_value=2)

        with pytest.raises(ForbiddenError, match="Rate limit exceeded"):
            await check_resource_limits(auth, mock_store, "dispatch")

    async def test_dispatch_rate_not_checked_for_vm_action(self) -> None:
        """max_dispatches_per_hour should only trigger for action='dispatch'."""
        limits = ResourceLimits(max_dispatches_per_hour=1)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_dispatches_since = AsyncMock(return_value=100)
        mock_store.count_active_vms = AsyncMock(return_value=0)

        # vm_provision should not check dispatch rate
        await check_resource_limits(auth, mock_store, "vm_provision")


class TestCheckResourceLimitsConcurrentVMs:
    """Test max_concurrent_vms enforcement."""

    async def test_under_vm_limit_passes(self) -> None:
        limits = ResourceLimits(max_concurrent_vms=3)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_dispatches_since = AsyncMock(return_value=0)
        mock_store.count_active_vms = AsyncMock(return_value=2)

        await check_resource_limits(auth, mock_store, "dispatch")

    async def test_at_vm_limit_raises_forbidden(self) -> None:
        limits = ResourceLimits(max_concurrent_vms=2)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_dispatches_since = AsyncMock(return_value=0)
        mock_store.count_active_vms = AsyncMock(return_value=2)

        with pytest.raises(ForbiddenError, match="VM limit exceeded"):
            await check_resource_limits(auth, mock_store, "dispatch")

    async def test_vm_limit_checked_for_vm_provision(self) -> None:
        limits = ResourceLimits(max_concurrent_vms=1)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_active_vms = AsyncMock(return_value=1)

        with pytest.raises(ForbiddenError, match="VM limit exceeded"):
            await check_resource_limits(auth, mock_store, "vm_provision")


class TestCheckResourceLimitsDailyCost:
    """Test max_cost_per_day enforcement."""

    async def test_under_cost_limit_passes(self) -> None:
        limits = ResourceLimits(max_cost_per_day=100.0)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_dispatches_since = AsyncMock(return_value=0)
        mock_store.count_active_vms = AsyncMock(return_value=0)
        mock_store.sum_cost_since = AsyncMock(return_value=50.0)

        await check_resource_limits(auth, mock_store, "dispatch")

    async def test_at_cost_limit_raises_forbidden(self) -> None:
        limits = ResourceLimits(max_cost_per_day=100.0)
        auth = _make_auth(resource_limits=limits)
        mock_store = AsyncMock()
        mock_store.count_dispatches_since = AsyncMock(return_value=0)
        mock_store.count_active_vms = AsyncMock(return_value=0)
        mock_store.sum_cost_since = AsyncMock(return_value=100.0)

        with pytest.raises(ForbiddenError, match="Cost limit exceeded"):
            await check_resource_limits(auth, mock_store, "dispatch")
