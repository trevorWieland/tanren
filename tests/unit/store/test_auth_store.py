"""Tests for AuthStore implementation on SqliteStore."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from pathlib import Path

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Dispatch, Phase
from tanren_core.store.enums import DispatchMode, Lane
from tanren_core.store.sqlite import SqliteStore

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_dispatch(workflow_id: str = "wf-test-1-100") -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=Phase.DO_TASK,
        project="test",
        spec_folder="spec/001",
        branch="main",
        cli=Cli.CLAUDE,
        auth=AuthMode.API_KEY,
        timeout=1800,
        resolved_profile=DEFAULT_PROFILE,
    )


@pytest.fixture
async def store(tmp_path: Path):
    s = SqliteStore(tmp_path / "auth_test.db")
    await s._ensure_conn()
    yield s
    await s.close()


# ── User tests ────────────────────────────────────────────────────────────


class TestCreateAndGetUser:
    async def test_create_and_get_user(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-001",
            name="Alice",
            email="alice@test.com",
            role="member",
        )
        user = await store.get_user("u-001")
        assert user is not None
        assert user.user_id == "u-001"
        assert user.name == "Alice"
        assert user.email == "alice@test.com"
        assert user.role == "member"
        assert user.is_active is True

    async def test_get_nonexistent_user_returns_none(self, store: SqliteStore) -> None:
        user = await store.get_user("nonexistent")
        assert user is None

    async def test_create_user_without_email(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-002",
            name="Bob",
            email=None,
            role="admin",
        )
        user = await store.get_user("u-002")
        assert user is not None
        assert user.email is None
        assert user.role == "admin"


class TestListUsers:
    async def test_list_empty(self, store: SqliteStore) -> None:
        users = await store.list_users()
        assert users == []

    async def test_list_users(self, store: SqliteStore) -> None:
        for i in range(3):
            await store.create_user(
                user_id=f"u-{i:03d}",
                name=f"User {i}",
                email=None,
                role="member",
            )
        users = await store.list_users()
        assert len(users) == 3

    async def test_list_users_pagination(self, store: SqliteStore) -> None:
        for i in range(5):
            await store.create_user(
                user_id=f"u-{i:03d}",
                name=f"User {i}",
                email=None,
                role="member",
            )
        page1 = await store.list_users(limit=2, offset=0)
        assert len(page1) == 2

        page2 = await store.list_users(limit=2, offset=2)
        assert len(page2) == 2

        page3 = await store.list_users(limit=2, offset=4)
        assert len(page3) == 1


class TestUpdateUser:
    async def test_update_name(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-update",
            name="Original",
            email=None,
            role="member",
        )
        await store.update_user("u-update", name="Updated")

        user = await store.get_user("u-update")
        assert user is not None
        assert user.name == "Updated"

    async def test_update_email(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-email",
            name="EmailUser",
            email=None,
            role="member",
        )
        await store.update_user("u-email", email="new@test.com")

        user = await store.get_user("u-email")
        assert user is not None
        assert user.email == "new@test.com"

    async def test_update_role(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-role",
            name="RoleUser",
            email=None,
            role="member",
        )
        await store.update_user("u-role", role="admin")

        user = await store.get_user("u-role")
        assert user is not None
        assert user.role == "admin"

    async def test_update_nothing_is_noop(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-noop",
            name="Noop",
            email=None,
            role="member",
        )
        await store.update_user("u-noop")

        user = await store.get_user("u-noop")
        assert user is not None
        assert user.name == "Noop"


class TestDeactivateUser:
    async def test_deactivate_user(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-deactivate",
            name="Deactivated",
            email=None,
            role="member",
        )
        await store.deactivate_user("u-deactivate")

        user = await store.get_user("u-deactivate")
        assert user is not None
        assert user.is_active is False


# ── API key tests ─────────────────────────────────────────────────────────


class TestCreateAndGetApiKey:
    async def test_create_and_get_by_hash(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-key-owner",
            name="KeyOwner",
            email=None,
            role="member",
        )
        await store.create_api_key(
            key_id="k-001",
            user_id="u-key-owner",
            name="test-key",
            key_prefix="abcd1234",
            key_hash="hash001",
            scopes_json=json.dumps(["dispatch:create", "dispatch:read"]),
            resource_limits_json=None,
            expires_at=None,
        )

        key = await store.get_api_key_by_hash("hash001")
        assert key is not None
        assert key.key_id == "k-001"
        assert key.user_id == "u-key-owner"
        assert key.name == "test-key"
        assert key.key_prefix == "abcd1234"
        assert key.key_hash == "hash001"
        assert key.scopes == ["dispatch:create", "dispatch:read"]
        assert key.resource_limits is None
        assert key.expires_at is None
        assert key.revoked_at is None

    async def test_get_by_hash_nonexistent_returns_none(self, store: SqliteStore) -> None:
        key = await store.get_api_key_by_hash("nonexistent-hash")
        assert key is None

    async def test_get_by_id(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-key-id",
            name="KeyIdOwner",
            email=None,
            role="member",
        )
        await store.create_api_key(
            key_id="k-byid",
            user_id="u-key-id",
            name="by-id-key",
            key_prefix="efgh5678",
            key_hash="hash-byid",
            scopes_json=json.dumps(["*"]),
            resource_limits_json=None,
            expires_at=None,
        )

        key = await store.get_api_key("k-byid")
        assert key is not None
        assert key.key_id == "k-byid"

    async def test_get_by_id_nonexistent_returns_none(self, store: SqliteStore) -> None:
        key = await store.get_api_key("nonexistent-key-id")
        assert key is None

    async def test_create_key_with_resource_limits(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-rl",
            name="RLOwner",
            email=None,
            role="member",
        )
        rl_json = json.dumps({
            "max_concurrent_vms": 2,
            "max_dispatches_per_hour": 10,
            "max_cost_per_day": 50.0,
        })
        await store.create_api_key(
            key_id="k-rl",
            user_id="u-rl",
            name="rl-key",
            key_prefix="rl123456",
            key_hash="hash-rl",
            scopes_json=json.dumps(["dispatch:create"]),
            resource_limits_json=rl_json,
            expires_at=None,
        )

        key = await store.get_api_key("k-rl")
        assert key is not None
        assert key.resource_limits is not None
        assert key.resource_limits.max_concurrent_vms == 2
        assert key.resource_limits.max_dispatches_per_hour == 10
        assert key.resource_limits.max_cost_per_day == pytest.approx(50.0)

    async def test_create_key_with_expires_at(self, store: SqliteStore) -> None:
        await store.create_user(
            user_id="u-exp",
            name="ExpiryOwner",
            email=None,
            role="member",
        )
        await store.create_api_key(
            key_id="k-exp",
            user_id="u-exp",
            name="expiry-key",
            key_prefix="exp12345",
            key_hash="hash-exp",
            scopes_json=json.dumps(["dispatch:read"]),
            resource_limits_json=None,
            expires_at="2030-12-31T23:59:59Z",
        )

        key = await store.get_api_key("k-exp")
        assert key is not None
        assert key.expires_at == "2030-12-31T23:59:59Z"


class TestListApiKeys:
    async def test_list_all_keys(self, store: SqliteStore) -> None:
        await store.create_user(user_id="u-list", name="ListUser", email=None, role="member")
        for i in range(3):
            await store.create_api_key(
                key_id=f"k-list-{i}",
                user_id="u-list",
                name=f"key-{i}",
                key_prefix=f"pf{i:06d}",
                key_hash=f"hash-list-{i}",
                scopes_json=json.dumps(["dispatch:read"]),
                resource_limits_json=None,
                expires_at=None,
            )

        keys = await store.list_api_keys()
        assert len(keys) == 3

    async def test_list_keys_by_user(self, store: SqliteStore) -> None:
        for uid in ["u-a", "u-b"]:
            await store.create_user(user_id=uid, name=uid, email=None, role="member")

        await store.create_api_key(
            key_id="k-a",
            user_id="u-a",
            name="key-a",
            key_prefix="pfx0000a",
            key_hash="hash-a",
            scopes_json=json.dumps(["*"]),
            resource_limits_json=None,
            expires_at=None,
        )
        await store.create_api_key(
            key_id="k-b",
            user_id="u-b",
            name="key-b",
            key_prefix="pfx0000b",
            key_hash="hash-b",
            scopes_json=json.dumps(["*"]),
            resource_limits_json=None,
            expires_at=None,
        )

        keys_a = await store.list_api_keys(user_id="u-a")
        assert len(keys_a) == 1
        assert keys_a[0].user_id == "u-a"

        keys_b = await store.list_api_keys(user_id="u-b")
        assert len(keys_b) == 1
        assert keys_b[0].user_id == "u-b"


class TestRevokeApiKey:
    async def test_revoke_sets_revoked_at(self, store: SqliteStore) -> None:
        await store.create_user(user_id="u-revoke", name="RevokeUser", email=None, role="member")
        await store.create_api_key(
            key_id="k-revoke",
            user_id="u-revoke",
            name="revoke-key",
            key_prefix="rev12345",
            key_hash="hash-revoke",
            scopes_json=json.dumps(["dispatch:read"]),
            resource_limits_json=None,
            expires_at=None,
        )

        await store.revoke_api_key("k-revoke")

        key = await store.get_api_key("k-revoke")
        assert key is not None
        assert key.revoked_at is not None

    async def test_revoked_key_excluded_from_default_list(self, store: SqliteStore) -> None:
        await store.create_user(user_id="u-rev-list", name="RevListUser", email=None, role="member")
        await store.create_api_key(
            key_id="k-rl1",
            user_id="u-rev-list",
            name="active-key",
            key_prefix="rl100000",
            key_hash="hash-rl1",
            scopes_json=json.dumps(["*"]),
            resource_limits_json=None,
            expires_at=None,
        )
        await store.create_api_key(
            key_id="k-rl2",
            user_id="u-rev-list",
            name="revoked-key",
            key_prefix="rl200000",
            key_hash="hash-rl2",
            scopes_json=json.dumps(["*"]),
            resource_limits_json=None,
            expires_at=None,
        )
        await store.revoke_api_key("k-rl2")

        keys = await store.list_api_keys(user_id="u-rev-list")
        assert len(keys) == 1
        assert keys[0].key_id == "k-rl1"


class TestSetGraceReplacement:
    async def test_grace_replacement(self, store: SqliteStore) -> None:
        await store.create_user(user_id="u-grace", name="GraceUser", email=None, role="member")
        await store.create_api_key(
            key_id="k-old",
            user_id="u-grace",
            name="old-key",
            key_prefix="old12345",
            key_hash="hash-old",
            scopes_json=json.dumps(["*"]),
            resource_limits_json=None,
            expires_at=None,
        )

        await store.set_grace_replacement(
            "k-old",
            replaced_by="k-new",
            revoked_at="2026-06-01T00:00:00Z",
        )

        key = await store.get_api_key("k-old")
        assert key is not None
        assert key.grace_replaced_by == "k-new"
        assert key.revoked_at == "2026-06-01T00:00:00Z"


# ── Resource limit query tests ────────────────────────────────────────────


class TestCountDispatchesSince:
    async def test_count_with_no_dispatches(self, store: SqliteStore) -> None:
        count = await store.count_dispatches_since("u-empty", "2026-01-01T00:00:00Z")
        assert count == 0

    async def test_count_dispatches_since_timestamp(self, store: SqliteStore) -> None:
        # Create dispatches with user_id
        for i in range(3):
            d = _make_dispatch(workflow_id=f"wf-count-{i}-100")
            await store.create_dispatch_projection(
                dispatch_id=f"wf-count-{i}-100",
                mode=DispatchMode.AUTO,
                lane=Lane.IMPL,
                preserve_on_failure=False,
                dispatch_json=d.model_dump_json(),
                user_id="u-counter",
            )

        count = await store.count_dispatches_since("u-counter", "2020-01-01T00:00:00Z")
        assert count == 3


class TestCountActiveVMs:
    async def test_no_active_vms(self, store: SqliteStore) -> None:
        count = await store.count_active_vms("u-novm")
        assert count == 0

    async def test_active_vm_counted(self, store: SqliteStore) -> None:
        # Create a dispatch with a completed provision step (no teardown)
        d = _make_dispatch(workflow_id="wf-vm-active")
        await store.create_dispatch_projection(
            dispatch_id="wf-vm-active",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=d.model_dump_json(),
            user_id="u-vm-owner",
        )
        await store.enqueue_step(
            step_id="step-prov-active",
            dispatch_id="wf-vm-active",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json="{}",
        )
        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        await store.ack(step.step_id, result_json='{"ok": true}')

        count = await store.count_active_vms("u-vm-owner")
        assert count == 1

    async def test_completed_teardown_not_counted(self, store: SqliteStore) -> None:
        # Create a dispatch with completed provision AND completed teardown
        d = _make_dispatch(workflow_id="wf-vm-torn")
        await store.create_dispatch_projection(
            dispatch_id="wf-vm-torn",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=d.model_dump_json(),
            user_id="u-vm-owner2",
        )
        # Provision step - completed
        await store.enqueue_step(
            step_id="step-prov-torn",
            dispatch_id="wf-vm-torn",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json="{}",
        )
        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        await store.ack(step.step_id, result_json='{"ok": true}')

        # Teardown step - completed
        await store.enqueue_step(
            step_id="step-tear-torn",
            dispatch_id="wf-vm-torn",
            step_type="teardown",
            step_sequence=2,
            lane=None,
            payload_json="{}",
        )
        step2 = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step2 is not None
        await store.ack(step2.step_id, result_json='{"ok": true}')

        count = await store.count_active_vms("u-vm-owner2")
        assert count == 0


class TestSumCostSince:
    async def test_no_cost_events(self, store: SqliteStore) -> None:
        cost = await store.sum_cost_since("u-nocost", "2026-01-01T00:00:00Z")
        assert cost == pytest.approx(0.0)
