"""Shared store protocol test classes — run against any backend.

Each test class uses a ``store`` fixture that must be provided by the
concrete test module (SQLite unit tests, Postgres integration tests, etc.).
This is the single source of truth for store contract tests.

Not discovered by pytest directly (underscore prefix); consumed by
backend-specific test files via class inheritance.
"""

from __future__ import annotations

import json
from typing import TYPE_CHECKING

import pytest

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Dispatch, Outcome, Phase
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
from tanren_core.store.events import DispatchCreated
from tanren_core.store.views import DispatchListFilter

if TYPE_CHECKING:
    from tanren_core.store.repository import Store

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_dispatch(
    workflow_id: str = "wf-test-1-100",
    phase: Phase = Phase.DO_TASK,
    cli: Cli = Cli.CLAUDE,
) -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=phase,
        project="test",
        spec_folder="spec/001",
        branch="main",
        cli=cli,
        auth=AuthMode.API_KEY,
        timeout=1800,
        resolved_profile=DEFAULT_PROFILE,
    )


# ═══════════════════════════════════════════════════════════════════════════
# EventStore protocol tests
# ═══════════════════════════════════════════════════════════════════════════


class SharedEventStoreTests:
    async def test_append_and_query(self, store: Store) -> None:
        dispatch = _make_dispatch()
        event = DispatchCreated(
            timestamp="2026-01-01T00:00:00Z",
            entity_id="wf-test-1-100",
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
        )
        await store.append(event)

        result = await store.query_events(entity_id="wf-test-1-100")
        assert result.total == 1
        assert len(result.events) == 1
        assert result.events[0].event_type == "DispatchCreated"
        assert result.events[0].entity_id == "wf-test-1-100"

    async def test_query_by_event_type(self, store: Store) -> None:
        event = DispatchCreated(
            timestamp="2026-01-01T00:00:00Z",
            entity_id="wf-test-1-100",
            dispatch=_make_dispatch(),
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
        )
        await store.append(event)

        result = await store.query_events(event_type="DispatchCreated")
        assert result.total == 1

        result = await store.query_events(event_type="StepEnqueued")
        assert result.total == 0

    async def test_query_with_time_range(self, store: Store) -> None:
        for i in range(3):
            event = DispatchCreated(
                timestamp=f"2026-01-0{i + 1}T00:00:00Z",
                entity_id=f"wf-test-{i}-100",
                dispatch=_make_dispatch(workflow_id=f"wf-test-{i}-100"),
                mode=DispatchMode.AUTO,
                lane=Lane.IMPL,
            )
            await store.append(event)

        result = await store.query_events(since="2026-01-02T00:00:00Z")
        assert result.total == 2

    async def test_query_pagination(self, store: Store) -> None:
        for i in range(5):
            event = DispatchCreated(
                timestamp=f"2026-01-01T00:0{i}:00Z",
                entity_id=f"wf-test-{i}-100",
                dispatch=_make_dispatch(workflow_id=f"wf-test-{i}-100"),
                mode=DispatchMode.AUTO,
                lane=Lane.IMPL,
            )
            await store.append(event)

        result = await store.query_events(limit=2, offset=0)
        assert len(result.events) == 2
        assert result.total == 5

        result = await store.query_events(limit=2, offset=3)
        assert len(result.events) == 2

    async def test_query_empty(self, store: Store) -> None:
        result = await store.query_events(entity_id="nonexistent")
        assert result.total == 0
        assert result.events == []


# ═══════════════════════════════════════════════════════════════════════════
# JobQueue protocol tests
# ═══════════════════════════════════════════════════════════════════════════


class SharedJobQueueTests:
    async def test_enqueue_and_dequeue(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json='{"test": true}',
        )

        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        assert step.step_id == "step-001"
        assert step.step_type == StepType.PROVISION
        assert step.dispatch_id == "wf-test-1-100"

    async def test_dequeue_empty(self, store: Store) -> None:
        step = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step is None

    async def test_dequeue_respects_lane(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        await store.enqueue_step(
            step_id="step-exec",
            dispatch_id="wf-test-1-100",
            step_type="execute",
            step_sequence=1,
            lane="impl",
            payload_json="{}",
        )

        # Different lane should find nothing
        step = await store.dequeue(lane=Lane.AUDIT, worker_id="w1", max_concurrent=1)
        assert step is None

        # Correct lane should find it
        step = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step is not None
        assert step.step_id == "step-exec"

    async def test_dequeue_respects_max_concurrent(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        for i in range(2):
            await store.enqueue_step(
                step_id=f"step-{i}",
                dispatch_id="wf-test-1-100",
                step_type="execute",
                step_sequence=i,
                lane="impl",
                payload_json="{}",
            )

        step1 = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step1 is not None

        step2 = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step2 is None

        await store.ack(step1.step_id, result_json='{"ok": true}')
        step2 = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step2 is not None

    async def test_ack_stores_result(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json="{}",
        )

        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        await store.ack(step.step_id, result_json='{"handle": "data"}')

        view = await store.get_step(step.step_id)
        assert view is not None
        assert view.status == StepStatus.COMPLETED
        assert view.result_json == '{"handle": "data"}'

    async def test_nack_marks_failed(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json="{}",
        )

        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        await store.nack(step.step_id, error="VM unavailable")

        view = await store.get_step(step.step_id)
        assert view is not None
        assert view.status == StepStatus.FAILED
        assert view.error == "VM unavailable"

    async def test_nack_with_retry(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="execute",
            step_sequence=1,
            lane="impl",
            payload_json="{}",
        )

        step = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step is not None
        await store.nack(step.step_id, error="transient error", retry=True)

        view = await store.get_step(step.step_id)
        assert view is not None
        assert view.status == StepStatus.PENDING
        assert view.retry_count == 1
        assert view.worker_id is None

        step2 = await store.dequeue(lane=Lane.IMPL, worker_id="w2", max_concurrent=1)
        assert step2 is not None
        assert step2.step_id == step.step_id


# ═══════════════════════════════════════════════════════════════════════════
# StateStore protocol tests
# ═══════════════════════════════════════════════════════════════════════════


class SharedStateStoreTests:
    async def test_create_and_get_dispatch(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        view = await store.get_dispatch("wf-test-1-100")
        assert view is not None
        assert view.dispatch_id == "wf-test-1-100"
        assert view.mode == DispatchMode.AUTO
        assert view.status == DispatchStatus.PENDING
        assert view.lane == Lane.IMPL
        assert view.dispatch.project == "test"

    async def test_get_nonexistent_dispatch(self, store: Store) -> None:
        view = await store.get_dispatch("nonexistent")
        assert view is None

    async def test_update_dispatch_status(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        await store.update_dispatch_status(
            "wf-test-1-100", DispatchStatus.COMPLETED, Outcome.SUCCESS
        )

        view = await store.get_dispatch("wf-test-1-100")
        assert view is not None
        assert view.status == DispatchStatus.COMPLETED
        assert view.outcome == Outcome.SUCCESS

    async def test_query_dispatches_by_status(self, store: Store) -> None:
        for i in range(3):
            d = _make_dispatch(workflow_id=f"wf-test-{i}-100")
            await store.create_dispatch_projection(
                dispatch_id=f"wf-test-{i}-100",
                mode=DispatchMode.AUTO,
                lane=Lane.IMPL,
                preserve_on_failure=False,
                dispatch_json=d.model_dump_json(),
            )

        await store.update_dispatch_status(
            "wf-test-0-100", DispatchStatus.COMPLETED, Outcome.SUCCESS
        )

        results = await store.query_dispatches(DispatchListFilter(status=DispatchStatus.PENDING))
        assert len(results) == 2

        results = await store.query_dispatches(DispatchListFilter(status=DispatchStatus.COMPLETED))
        assert len(results) == 1

    async def test_get_steps_for_dispatch(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        for i, (st, lane) in enumerate([
            ("provision", None),
            ("execute", "impl"),
            ("teardown", None),
        ]):
            await store.enqueue_step(
                step_id=f"step-{i}",
                dispatch_id="wf-test-1-100",
                step_type=st,
                step_sequence=i,
                lane=lane,
                payload_json="{}",
            )

        steps = await store.get_steps_for_dispatch("wf-test-1-100")
        assert len(steps) == 3
        assert steps[0].step_type == StepType.PROVISION
        assert steps[1].step_type == StepType.EXECUTE
        assert steps[2].step_type == StepType.TEARDOWN

    async def test_count_running_steps(self, store: Store) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="execute",
            step_sequence=1,
            lane="impl",
            payload_json="{}",
        )

        assert await store.count_running_steps(lane=Lane.IMPL) == 0

        await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert await store.count_running_steps(lane=Lane.IMPL) == 1


class SharedLifecycleTests:
    async def test_close(self, store: Store) -> None:
        await store.close()
        await store.close()

    async def test_full_dispatch_lifecycle(self, store: Store) -> None:
        """End-to-end: create dispatch -> enqueue provision -> dequeue -> ack."""
        dispatch = _make_dispatch()
        dispatch_json = dispatch.model_dump_json()

        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch_json,
        )

        event = DispatchCreated(
            timestamp="2026-01-01T00:00:00Z",
            entity_id="wf-test-1-100",
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
        )
        await store.append(event)

        payload = json.dumps({"dispatch": json.loads(dispatch_json)})
        await store.enqueue_step(
            step_id="step-prov",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=payload,
        )

        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        assert step.step_id == "step-prov"

        await store.ack(step.step_id, result_json='{"handle": {"env_id": "e1"}}')

        step_view = await store.get_step("step-prov")
        assert step_view is not None
        assert step_view.status == StepStatus.COMPLETED

        dispatch_view = await store.get_dispatch("wf-test-1-100")
        assert dispatch_view is not None
        assert dispatch_view.status == DispatchStatus.RUNNING

        events = await store.query_events(entity_id="wf-test-1-100")
        assert events.total >= 2


# ═══════════════════════════════════════════════════════════════════════════
# AuthStore protocol tests
# ═══════════════════════════════════════════════════════════════════════════


class SharedUserTests:
    async def test_create_and_get_user(self, store: Store) -> None:
        await store.create_user(
            user_id="u-001", name="Alice", email="alice@test.com", role="member"
        )
        user = await store.get_user("u-001")
        assert user is not None
        assert user.user_id == "u-001"
        assert user.name == "Alice"
        assert user.email == "alice@test.com"
        assert user.role == "member"
        assert user.is_active is True

    async def test_get_nonexistent_user_returns_none(self, store: Store) -> None:
        user = await store.get_user("nonexistent")
        assert user is None

    async def test_create_user_without_email(self, store: Store) -> None:
        await store.create_user(user_id="u-002", name="Bob", email=None, role="admin")
        user = await store.get_user("u-002")
        assert user is not None
        assert user.email is None
        assert user.role == "admin"

    async def test_list_empty(self, store: Store) -> None:
        users = await store.list_users()
        assert users == []

    async def test_list_users(self, store: Store) -> None:
        for i in range(3):
            await store.create_user(
                user_id=f"u-{i:03d}", name=f"User {i}", email=None, role="member"
            )
        users = await store.list_users()
        assert len(users) == 3

    async def test_list_users_pagination(self, store: Store) -> None:
        for i in range(5):
            await store.create_user(
                user_id=f"u-{i:03d}", name=f"User {i}", email=None, role="member"
            )
        assert len(await store.list_users(limit=2, offset=0)) == 2
        assert len(await store.list_users(limit=2, offset=2)) == 2
        assert len(await store.list_users(limit=2, offset=4)) == 1

    async def test_update_name(self, store: Store) -> None:
        await store.create_user(user_id="u-update", name="Original", email=None, role="member")
        await store.update_user("u-update", name="Updated")
        user = await store.get_user("u-update")
        assert user is not None
        assert user.name == "Updated"

    async def test_update_email(self, store: Store) -> None:
        await store.create_user(user_id="u-email", name="EmailUser", email=None, role="member")
        await store.update_user("u-email", email="new@test.com")
        user = await store.get_user("u-email")
        assert user is not None
        assert user.email == "new@test.com"

    async def test_update_role(self, store: Store) -> None:
        await store.create_user(user_id="u-role", name="RoleUser", email=None, role="member")
        await store.update_user("u-role", role="admin")
        user = await store.get_user("u-role")
        assert user is not None
        assert user.role == "admin"

    async def test_deactivate_user(self, store: Store) -> None:
        await store.create_user(
            user_id="u-deactivate", name="Deactivated", email=None, role="member"
        )
        await store.deactivate_user("u-deactivate")
        user = await store.get_user("u-deactivate")
        assert user is not None
        assert user.is_active is False


class SharedApiKeyTests:
    async def test_create_and_get_by_hash(self, store: Store) -> None:
        await store.create_user(user_id="u-key-owner", name="KeyOwner", email=None, role="member")
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
        assert key.scopes == ["dispatch:create", "dispatch:read"]
        assert key.resource_limits is None

    async def test_get_by_hash_nonexistent_returns_none(self, store: Store) -> None:
        key = await store.get_api_key_by_hash("nonexistent-hash")
        assert key is None

    async def test_get_by_id(self, store: Store) -> None:
        await store.create_user(user_id="u-key-id", name="KeyIdOwner", email=None, role="member")
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

    async def test_create_key_with_resource_limits(self, store: Store) -> None:
        await store.create_user(user_id="u-rl", name="RLOwner", email=None, role="member")
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

    async def test_create_key_with_expires_at(self, store: Store) -> None:
        await store.create_user(user_id="u-exp", name="ExpiryOwner", email=None, role="member")
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

    async def test_list_all_keys(self, store: Store) -> None:
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

    async def test_list_keys_by_user(self, store: Store) -> None:
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

    async def test_revoke_sets_revoked_at(self, store: Store) -> None:
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

    async def test_revoked_key_excluded_from_default_list(self, store: Store) -> None:
        await store.create_user(user_id="u-rev-list", name="RevListUser", email=None, role="member")
        for kid, name, prefix, khash in [
            ("k-rl1", "active-key", "rl100000", "hash-rl1"),
            ("k-rl2", "revoked-key", "rl200000", "hash-rl2"),
        ]:
            await store.create_api_key(
                key_id=kid,
                user_id="u-rev-list",
                name=name,
                key_prefix=prefix,
                key_hash=khash,
                scopes_json=json.dumps(["*"]),
                resource_limits_json=None,
                expires_at=None,
            )
        await store.revoke_api_key("k-rl2")
        keys = await store.list_api_keys(user_id="u-rev-list")
        assert len(keys) == 1
        assert keys[0].key_id == "k-rl1"

    async def test_grace_replacement(self, store: Store) -> None:
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
            "k-old", replaced_by="k-new", revoked_at="2026-06-01T00:00:00Z"
        )
        key = await store.get_api_key("k-old")
        assert key is not None
        assert key.grace_replaced_by == "k-new"
        assert key.revoked_at == "2026-06-01T00:00:00Z"


class SharedResourceLimitTests:
    async def test_count_with_no_dispatches(self, store: Store) -> None:
        count = await store.count_dispatches_since("u-empty", "2026-01-01T00:00:00Z")
        assert count == 0

    async def test_count_dispatches_since_timestamp(self, store: Store) -> None:
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

    async def test_no_active_vms(self, store: Store) -> None:
        count = await store.count_active_vms("u-novm")
        assert count == 0

    async def test_active_vm_counted(self, store: Store) -> None:
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

    async def test_completed_teardown_not_counted(self, store: Store) -> None:
        d = _make_dispatch(workflow_id="wf-vm-torn")
        await store.create_dispatch_projection(
            dispatch_id="wf-vm-torn",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=d.model_dump_json(),
            user_id="u-vm-owner2",
        )
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

    async def test_no_cost_events(self, store: Store) -> None:
        cost = await store.sum_cost_since("u-nocost", "2026-01-01T00:00:00Z")
        assert cost == pytest.approx(0.0)
