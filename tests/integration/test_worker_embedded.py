"""Integration test: embedded Worker step processing."""

import uuid
from datetime import UTC, datetime
from pathlib import Path
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.types import EnvironmentHandle, LocalEnvironmentRuntime, PhaseResult
from tanren_core.ccusage import TokenUsage
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase
from tanren_core.store.enums import DispatchMode, DispatchStatus, StepStatus, StepType, cli_to_lane
from tanren_core.store.events import DispatchCreated
from tanren_core.store.factory import create_sqlite_store
from tanren_core.store.payloads import ProvisionStepPayload
from tanren_core.worker import Worker
from tanren_core.worker_config import WorkerConfig

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_config(tmp_path):
    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    return WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
        roles_config_path=str(roles_yml),
        db_url=str(tmp_path / "test.db"),
        poll_interval_secs=0.1,
    )


def _make_dispatch():
    return Dispatch(
        workflow_id=f"wf-test-{uuid.uuid4().hex[:8]}",
        phase=Phase.DO_TASK,
        project="test",
        spec_folder="specs/001",
        branch="main",
        cli=Cli.CLAUDE,
        timeout=1800,
        resolved_profile=DEFAULT_PROFILE,
    )


@pytest.mark.asyncio
async def test_process_step_provision_auto_chains(tmp_path):
    """Worker.process_step() provision in AUTO mode auto-chains to execute."""
    config = _make_config(tmp_path)
    store = await create_sqlite_store(str(tmp_path / "test.db"))

    mock_env = AsyncMock()
    handle = EnvironmentHandle(
        env_id="env-1",
        worktree_path=Path("/tmp/wt"),
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(task_env={}),
    )
    mock_env.provision = AsyncMock(return_value=handle)

    worker = Worker(
        config=config,
        event_store=store,
        job_queue=store,
        state_store=store,
        env_factory=lambda cfg, prof: (mock_env, None),
    )

    dispatch = _make_dispatch()
    dispatch_id = dispatch.workflow_id
    lane = cli_to_lane(dispatch.cli)

    # Create dispatch projection + enqueue provision step
    await store.create_dispatch_projection(
        dispatch_id=dispatch_id,
        mode=DispatchMode.AUTO,
        lane=lane,
        preserve_on_failure=False,
        dispatch_json=dispatch.model_dump_json(),
    )
    await store.append(
        DispatchCreated(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch_id,
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=lane,
        )
    )
    step_id = uuid.uuid4().hex
    await store.enqueue_step(
        step_id=step_id,
        dispatch_id=dispatch_id,
        step_type="provision",
        step_sequence=0,
        lane=None,
        payload_json=ProvisionStepPayload(dispatch=dispatch).model_dump_json(),
    )

    # Dequeue and process the provision step
    step = await store.dequeue(lane=None, worker_id="test-worker", max_concurrent=10)
    assert step is not None
    assert step.step_type == StepType.PROVISION

    await worker.process_step(step)

    # Verify provision was called
    mock_env.provision.assert_awaited_once()

    # In AUTO mode, an execute step should have been auto-chained
    steps = await store.get_steps_for_dispatch(dispatch_id)
    assert len(steps) == 2
    provision_step = next(s for s in steps if s.step_type == StepType.PROVISION)
    execute_step = next(s for s in steps if s.step_type == StepType.EXECUTE)
    assert provision_step.status == StepStatus.COMPLETED
    assert execute_step.status == StepStatus.PENDING

    # Verify provision result includes dispatch_id for handle reconstruction
    from tanren_core.store.payloads import ProvisionResult

    assert provision_step.result_json is not None
    prov_result = ProvisionResult.model_validate_json(provision_step.result_json)
    assert prov_result.handle.dispatch_id == dispatch_id

    await store.close()


@pytest.mark.asyncio
async def test_full_auto_chain_provision_execute_teardown(tmp_path):
    """Worker processes provision -> execute -> teardown via sequential process_step."""
    config = _make_config(tmp_path)
    store = await create_sqlite_store(str(tmp_path / "chain.db"))

    mock_env = AsyncMock()
    handle = EnvironmentHandle(
        env_id="env-1",
        worktree_path=Path("/tmp/wt"),
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(task_env={}),
    )
    mock_env.provision = AsyncMock(return_value=handle)
    mock_env.execute = AsyncMock(
        return_value=PhaseResult(
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            stdout="ok",
            duration_secs=1,
            preflight_passed=True,
        )
    )
    mock_env.teardown = AsyncMock()

    worker = Worker(
        config=config,
        event_store=store,
        job_queue=store,
        state_store=store,
        env_factory=lambda cfg, prof: (mock_env, None),
    )

    dispatch = _make_dispatch()
    dispatch_id = dispatch.workflow_id
    lane = cli_to_lane(dispatch.cli)

    await store.create_dispatch_projection(
        dispatch_id=dispatch_id,
        mode=DispatchMode.AUTO,
        lane=lane,
        preserve_on_failure=False,
        dispatch_json=dispatch.model_dump_json(),
    )
    await store.append(
        DispatchCreated(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch_id,
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=lane,
        )
    )
    await store.enqueue_step(
        step_id=uuid.uuid4().hex,
        dispatch_id=dispatch_id,
        step_type="provision",
        step_sequence=0,
        lane=None,
        payload_json=ProvisionStepPayload(dispatch=dispatch).model_dump_json(),
    )

    # Step 1: provision (lane=None)
    step = await store.dequeue(lane=None, worker_id="test-worker", max_concurrent=10)
    assert step is not None
    await worker.process_step(step)
    mock_env.provision.assert_awaited_once()

    # Step 2: execute (auto-chained, lane=impl)
    step = await store.dequeue(lane=lane, worker_id="test-worker", max_concurrent=10)
    assert step is not None
    assert step.step_type == StepType.EXECUTE
    await worker.process_step(step)
    mock_env.execute.assert_awaited_once()

    # Step 3: teardown (auto-chained, lane=None)
    step = await store.dequeue(lane=None, worker_id="test-worker", max_concurrent=10)
    assert step is not None
    assert step.step_type == StepType.TEARDOWN
    await worker.process_step(step)
    mock_env.teardown.assert_awaited_once()

    # Verify dispatch is completed
    view = await store.get_dispatch(dispatch_id)
    assert view is not None
    assert view.status == DispatchStatus.COMPLETED

    await store.close()


@pytest.mark.asyncio
async def test_process_step_provision_manual_no_chain(tmp_path):
    """Worker.process_step() provision in MANUAL mode does NOT auto-chain."""
    config = _make_config(tmp_path)
    store = await create_sqlite_store(str(tmp_path / "manual.db"))

    mock_env = AsyncMock()
    handle = EnvironmentHandle(
        env_id="env-2",
        worktree_path=Path("/tmp/wt"),
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(task_env={}),
    )
    mock_env.provision = AsyncMock(return_value=handle)

    worker = Worker(
        config=config,
        event_store=store,
        job_queue=store,
        state_store=store,
        env_factory=lambda cfg, prof: (mock_env, None),
    )

    dispatch = _make_dispatch()
    dispatch_id = dispatch.workflow_id
    lane = cli_to_lane(dispatch.cli)

    await store.create_dispatch_projection(
        dispatch_id=dispatch_id,
        mode=DispatchMode.MANUAL,
        lane=lane,
        preserve_on_failure=True,
        dispatch_json=dispatch.model_dump_json(),
    )
    await store.append(
        DispatchCreated(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch_id,
            dispatch=dispatch,
            mode=DispatchMode.MANUAL,
            lane=lane,
        )
    )
    await store.enqueue_step(
        step_id=uuid.uuid4().hex,
        dispatch_id=dispatch_id,
        step_type="provision",
        step_sequence=0,
        lane=None,
        payload_json=ProvisionStepPayload(dispatch=dispatch).model_dump_json(),
    )

    # Dequeue and process provision step
    step = await store.dequeue(lane=None, worker_id="test-worker", max_concurrent=10)
    assert step is not None
    await worker.process_step(step)

    mock_env.provision.assert_awaited_once()

    # In MANUAL mode, NO execute step should have been auto-chained
    steps = await store.get_steps_for_dispatch(dispatch_id)
    assert len(steps) == 1
    assert steps[0].step_type == StepType.PROVISION
    assert steps[0].status == StepStatus.COMPLETED

    # execute/teardown NOT called
    mock_env.execute.assert_not_awaited()
    mock_env.teardown.assert_not_awaited()

    await store.close()


@pytest.mark.asyncio
async def test_execute_with_token_usage_emits_event(tmp_path):
    """Worker emits TokenUsageRecorded event when execute captures usage."""
    config = _make_config(tmp_path)
    store = await create_sqlite_store(str(tmp_path / "token.db"))

    mock_env = AsyncMock()
    handle = EnvironmentHandle(
        env_id="env-tok",
        worktree_path=Path("/tmp/wt"),
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(task_env={}),
    )
    mock_env.provision = AsyncMock(return_value=handle)
    mock_env.execute = AsyncMock(
        return_value=PhaseResult(
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            stdout="ok",
            duration_secs=5,
            preflight_passed=True,
            token_usage=TokenUsage(
                input_tokens=1000,
                output_tokens=500,
                total_tokens=1500,
                total_cost=0.015,
                provider="claude",
            ),
        )
    )
    mock_env.teardown = AsyncMock()

    worker = Worker(
        config=config,
        event_store=store,
        job_queue=store,
        state_store=store,
        env_factory=lambda cfg, prof: (mock_env, None),
    )

    dispatch = _make_dispatch()
    dispatch_id = dispatch.workflow_id
    lane = cli_to_lane(dispatch.cli)

    await store.create_dispatch_projection(
        dispatch_id=dispatch_id,
        mode=DispatchMode.AUTO,
        lane=lane,
        preserve_on_failure=False,
        dispatch_json=dispatch.model_dump_json(),
    )
    await store.append(
        DispatchCreated(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch_id,
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=lane,
        )
    )
    await store.enqueue_step(
        step_id=uuid.uuid4().hex,
        dispatch_id=dispatch_id,
        step_type="provision",
        step_sequence=0,
        lane=None,
        payload_json=ProvisionStepPayload(dispatch=dispatch).model_dump_json(),
    )

    # Step 1: provision
    step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
    assert step is not None
    await worker.process_step(step)

    # Step 2: execute (auto-chained)
    step = await store.dequeue(lane=lane, worker_id="w1", max_concurrent=10)
    assert step is not None
    await worker.process_step(step)

    # Verify TokenUsageRecorded event was emitted
    events = await store.query_events(dispatch_id=dispatch_id)
    event_types = [e.event_type for e in events.events]
    assert "TokenUsageRecorded" in event_types

    # Process teardown to complete the lifecycle
    step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
    assert step is not None
    assert step.step_type == StepType.TEARDOWN
    await worker.process_step(step)

    # Verify dispatch completed with events
    view = await store.get_dispatch(dispatch_id)
    assert view is not None
    assert view.status == DispatchStatus.COMPLETED

    all_events = await store.query_events(dispatch_id=dispatch_id)
    all_types = [e.event_type for e in all_events.events]
    assert "DispatchCompleted" in all_types

    await store.close()


@pytest.mark.asyncio
async def test_execute_failure_enqueues_cleanup_teardown(tmp_path):
    """When execute raises unexpectedly in AUTO mode, a teardown step is enqueued."""
    config = _make_config(tmp_path)
    store = await create_sqlite_store(str(tmp_path / "fail.db"))

    mock_env = AsyncMock()
    handle = EnvironmentHandle(
        env_id="env-fail",
        worktree_path=Path("/tmp/wt"),
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(task_env={}),
    )
    mock_env.provision = AsyncMock(return_value=handle)
    mock_env.execute = AsyncMock(side_effect=RuntimeError("boom"))
    mock_env.teardown = AsyncMock()

    worker = Worker(
        config=config,
        event_store=store,
        job_queue=store,
        state_store=store,
        env_factory=lambda cfg, prof: (mock_env, None),
    )

    dispatch = _make_dispatch()
    dispatch_id = dispatch.workflow_id
    lane = cli_to_lane(dispatch.cli)

    await store.create_dispatch_projection(
        dispatch_id=dispatch_id,
        mode=DispatchMode.AUTO,
        lane=lane,
        preserve_on_failure=False,
        dispatch_json=dispatch.model_dump_json(),
    )
    await store.append(
        DispatchCreated(
            timestamp=datetime.now(UTC).isoformat(),
            workflow_id=dispatch_id,
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=lane,
        )
    )
    await store.enqueue_step(
        step_id=uuid.uuid4().hex,
        dispatch_id=dispatch_id,
        step_type="provision",
        step_sequence=0,
        lane=None,
        payload_json=ProvisionStepPayload(dispatch=dispatch).model_dump_json(),
    )

    # Provision (auto-chains to execute)
    step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
    assert step is not None
    await worker.process_step(step)

    # Execute (will raise RuntimeError → _handle_step_failure → enqueue teardown)
    step = await store.dequeue(lane=lane, worker_id="w1", max_concurrent=10)
    assert step is not None
    assert step.step_type == StepType.EXECUTE
    await worker.process_step(step)

    # A teardown step should have been enqueued for cleanup
    steps = await store.get_steps_for_dispatch(dispatch_id)
    teardown_steps = [s for s in steps if s.step_type == StepType.TEARDOWN]
    assert len(teardown_steps) == 1
    assert teardown_steps[0].status == StepStatus.PENDING

    await store.close()
