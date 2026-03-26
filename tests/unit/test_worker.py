"""Tests for the queue-consuming Worker class."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING
from unittest.mock import AsyncMock, MagicMock

if TYPE_CHECKING:
    from pathlib import Path

from tanren_core.adapters.types import (
    EnvironmentHandle,
    LocalEnvironmentRuntime,
    PhaseResult,
)
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Dispatch, Outcome, Phase
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
from tanren_core.store.handle import PersistedEnvironmentHandle
from tanren_core.store.payloads import (
    ExecuteStepPayload,
    ProvisionStepPayload,
    TeardownStepPayload,
)
from tanren_core.store.views import DispatchView, QueuedStep, StepView
from tanren_core.worker import Worker

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


def _make_handle() -> PersistedEnvironmentHandle:
    return PersistedEnvironmentHandle(
        env_id="env-abc",
        worktree_path="/workspace/test",
        branch="main",
        project="test",
        provision_timestamp="2026-01-01T00:00:00Z",
    )


def _make_config(tmp_path: Path) -> MagicMock:
    config = MagicMock()
    config.max_provision = 10
    config.max_impl = 1
    config.max_audit = 1
    config.max_gate = 3
    config.poll_interval_secs = 0.1
    config.worker_id = "test-worker"
    config.github_dir = str(tmp_path / "github")
    config.data_dir = str(tmp_path / "data")
    config.commands_dir = ".claude/commands/tanren"
    config.opencode_path = "opencode"
    config.codex_path = "codex"
    config.claude_path = "claude"
    config.roles_config_path = str(tmp_path / "roles.yml")
    config.ipc_dir = str(tmp_path / "ipc")
    return config


def _make_phase_result(outcome: Outcome = Outcome.SUCCESS) -> PhaseResult:
    return PhaseResult(
        outcome=outcome,
        exit_code=0 if outcome == Outcome.SUCCESS else 1,
        duration_secs=10,
        preflight_passed=True,
        plan_hash="abcd1234",
    )


def _make_env_handle(tmp_path: Path) -> EnvironmentHandle:
    return EnvironmentHandle(
        env_id="env-abc",
        worktree_path=tmp_path / "workspace",
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(task_env={}),
    )


def _make_dispatch_view(
    dispatch_id: str = "wf-test-1-100",
    mode: DispatchMode = DispatchMode.AUTO,
    status: DispatchStatus = DispatchStatus.RUNNING,
) -> DispatchView:
    return DispatchView(
        dispatch_id=dispatch_id,
        mode=mode,
        status=status,
        outcome=None,
        lane=Lane.IMPL,
        preserve_on_failure=False,
        dispatch=_make_dispatch(dispatch_id),
        user_id="",
        created_at="2026-01-01T00:00:00Z",
        updated_at="2026-01-01T00:00:00Z",
    )


def _make_env_factory(execution_env):
    """Create an env_factory that returns the given mock execution_env."""

    def factory(config, profile):
        return execution_env, None

    return factory


class TestWorkerProcessStep:
    async def test_provision_step_calls_execution_env(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        execution_env = AsyncMock()
        execution_env.provision = AsyncMock(return_value=_make_env_handle(tmp_path))

        event_store = AsyncMock()
        job_queue = AsyncMock()
        state_store = AsyncMock()
        state_store.get_dispatch = AsyncMock(
            return_value=_make_dispatch_view(mode=DispatchMode.AUTO)
        )

        worker = Worker(
            config=config,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
            env_factory=_make_env_factory(execution_env),
        )

        dispatch = _make_dispatch()
        payload = ProvisionStepPayload(dispatch=dispatch)
        step = QueuedStep(
            step_id="step-prov",
            dispatch_id="wf-test-1-100",
            step_type=StepType.PROVISION,
            step_sequence=0,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        await worker.process_step(step)

        execution_env.provision.assert_called_once()
        # Auto-chain: should atomically ack + enqueue execute step
        job_queue.ack_and_enqueue.assert_called_once()
        enqueue_call = job_queue.ack_and_enqueue.call_args
        assert enqueue_call.kwargs["next_step_type"] == "execute"
        # Completion event should be included in the atomic transaction
        assert "completion_events" in enqueue_call.kwargs
        completion_events = enqueue_call.kwargs["completion_events"]
        assert len(completion_events) == 1
        assert type(completion_events[0]).__name__ == "StepCompleted"

    async def test_execute_step_calls_execution_env(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        execution_env = AsyncMock()
        execution_env.execute = AsyncMock(return_value=_make_phase_result())

        event_store = AsyncMock()
        job_queue = AsyncMock()
        state_store = AsyncMock()
        state_store.get_dispatch = AsyncMock(
            return_value=_make_dispatch_view(mode=DispatchMode.AUTO)
        )

        worker = Worker(
            config=config,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
            env_factory=_make_env_factory(execution_env),
        )

        dispatch = _make_dispatch()
        handle = _make_handle()
        payload = ExecuteStepPayload(dispatch=dispatch, handle=handle)
        step = QueuedStep(
            step_id="step-exec",
            dispatch_id="wf-test-1-100",
            step_type=StepType.EXECUTE,
            step_sequence=1,
            lane=Lane.IMPL,
            payload_json=payload.model_dump_json(),
        )

        await worker.process_step(step)

        execution_env.execute.assert_called_once()
        # Auto-chain: should atomically ack + enqueue teardown step
        job_queue.ack_and_enqueue.assert_called_once()
        enqueue_call = job_queue.ack_and_enqueue.call_args
        assert enqueue_call.kwargs["next_step_type"] == "teardown"
        # Completion events should be included in the atomic transaction
        assert "completion_events" in enqueue_call.kwargs
        completion_events = enqueue_call.kwargs["completion_events"]
        assert len(completion_events) == 2
        event_types = [type(e).__name__ for e in completion_events]
        assert event_types == ["StepCompleted", "PhaseCompleted"]

    async def test_teardown_step_calls_execution_env(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        execution_env = AsyncMock()

        event_store = AsyncMock()
        job_queue = AsyncMock()

        execute_step_view = StepView(
            step_id="step-exec",
            dispatch_id="wf-test-1-100",
            step_type=StepType.EXECUTE,
            step_sequence=1,
            lane=Lane.IMPL,
            status=StepStatus.COMPLETED,
            worker_id="w1",
            result_json=json.dumps({
                "outcome": "success",
                "exit_code": 0,
                "duration_secs": 10,
            }),
            error=None,
            retry_count=0,
            created_at="2026-01-01T00:00:00Z",
            updated_at="2026-01-01T00:00:10Z",
        )

        state_store = AsyncMock()
        state_store.get_dispatch = AsyncMock(return_value=_make_dispatch_view())
        state_store.get_steps_for_dispatch = AsyncMock(return_value=[execute_step_view])

        worker = Worker(
            config=config,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
            env_factory=_make_env_factory(execution_env),
        )

        dispatch = _make_dispatch()
        handle = _make_handle()
        payload = TeardownStepPayload(dispatch=dispatch, handle=handle)
        step = QueuedStep(
            step_id="step-td",
            dispatch_id="wf-test-1-100",
            step_type=StepType.TEARDOWN,
            step_sequence=2,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        await worker.process_step(step)

        execution_env.teardown.assert_called_once()
        job_queue.ack.assert_called_once()
        # Should NOT enqueue anything after teardown
        job_queue.enqueue_step.assert_not_called()
        # Should mark dispatch completed
        state_store.update_dispatch_status.assert_called_once()

    async def test_teardown_with_preserve_skips_execution_env(
        self,
        tmp_path: Path,
    ) -> None:
        config = _make_config(tmp_path)
        execution_env = AsyncMock()
        event_store = AsyncMock()
        job_queue = AsyncMock()
        state_store = AsyncMock()
        state_store.get_dispatch = AsyncMock(return_value=_make_dispatch_view())
        state_store.get_steps_for_dispatch = AsyncMock(return_value=[])

        worker = Worker(
            config=config,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
            env_factory=_make_env_factory(execution_env),
        )

        dispatch = _make_dispatch()
        handle = _make_handle()
        payload = TeardownStepPayload(dispatch=dispatch, handle=handle, preserve=True)
        step = QueuedStep(
            step_id="step-td",
            dispatch_id="wf-test-1-100",
            step_type=StepType.TEARDOWN,
            step_sequence=2,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        await worker.process_step(step)

        # Preserve=True: should NOT call teardown
        execution_env.teardown.assert_not_called()
        # But should still ack the step
        job_queue.ack.assert_called_once()


class TestWorkerManualMode:
    async def test_manual_mode_does_not_auto_chain(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        execution_env = AsyncMock()
        execution_env.provision = AsyncMock(return_value=_make_env_handle(tmp_path))

        event_store = AsyncMock()
        job_queue = AsyncMock()
        state_store = AsyncMock()
        state_store.get_dispatch = AsyncMock(
            return_value=_make_dispatch_view(mode=DispatchMode.MANUAL)
        )

        worker = Worker(
            config=config,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
            env_factory=_make_env_factory(execution_env),
        )

        dispatch = _make_dispatch()
        payload = ProvisionStepPayload(dispatch=dispatch)
        step = QueuedStep(
            step_id="step-prov",
            dispatch_id="wf-test-1-100",
            step_type=StepType.PROVISION,
            step_sequence=0,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        await worker.process_step(step)

        execution_env.provision.assert_called_once()
        job_queue.ack.assert_called_once()
        # Manual mode: should NOT auto-chain
        job_queue.enqueue_step.assert_not_called()


class TestWorkerHandlePersistence:
    def test_persist_local_handle(self, tmp_path: Path) -> None:
        handle = _make_env_handle(tmp_path)
        persisted = Worker._persist_handle(handle, profile_name="default")

        assert persisted.env_id == "env-abc"
        assert persisted.project == "test"
        assert persisted.vm is None
        assert persisted.ssh_config is None
        assert persisted.profile_name == "default"

    def test_reconstruct_local_handle(self) -> None:
        persisted = _make_handle()
        handle = Worker._reconstruct_handle(persisted)

        assert handle.env_id == "env-abc"
        assert handle.project == "test"
        assert handle.runtime.kind == "local"

    def test_persist_roundtrip(self, tmp_path: Path) -> None:
        original = _make_env_handle(tmp_path)
        persisted = Worker._persist_handle(original, profile_name="default")
        reconstructed = Worker._reconstruct_handle(persisted)

        assert reconstructed.env_id == original.env_id
        assert reconstructed.project == original.project
        assert reconstructed.branch == original.branch

    def test_persist_docker_handle(self, tmp_path: Path) -> None:
        from unittest.mock import MagicMock as _MM

        from tanren_core.adapters.remote_types import WorkspacePath
        from tanren_core.adapters.types import DockerEnvironmentRuntime

        conn = _MM()
        workspace = WorkspacePath(path="/workspace/proj", project="proj", branch="main")
        handle = EnvironmentHandle(
            env_id="env-docker",
            worktree_path=tmp_path / "workspace",
            branch="main",
            project="proj",
            runtime=DockerEnvironmentRuntime(
                container_id="abc123def456",
                connection=conn,
                workspace_path=workspace,
                profile=EnvironmentProfile(name="docker-ci"),
                provision_start=100.0,
                workflow_id="wf-1",
                docker_socket_url="unix:///var/run/docker.sock",
            ),
        )
        persisted = Worker._persist_handle(handle, profile_name="docker-ci")

        assert persisted.docker_config is not None
        assert persisted.docker_config.container_id == "abc123def456"
        assert persisted.docker_config.socket_url == "unix:///var/run/docker.sock"
        assert persisted.workspace_remote_path == "/workspace/proj"
        assert persisted.profile_name == "docker-ci"
        assert persisted.vm is None
        assert persisted.ssh_config is None

    def test_reconstruct_docker_handle(self) -> None:
        from tanren_core.store.handle import PersistedDockerConfig

        persisted = PersistedEnvironmentHandle(
            env_id="env-docker",
            worktree_path="/workspace/proj",
            branch="main",
            project="proj",
            provision_timestamp="2026-01-01T00:00:00Z",
            workspace_remote_path="/workspace/proj",
            profile_name="docker-ci",
            docker_config=PersistedDockerConfig(
                container_id="abc123def456",
                socket_url=None,
            ),
        )
        handle = Worker._reconstruct_handle(persisted)

        from tanren_core.adapters.types import DockerEnvironmentRuntime

        assert handle.env_id == "env-docker"
        assert handle.runtime.kind == "docker"
        assert isinstance(handle.runtime, DockerEnvironmentRuntime)
        assert handle.runtime.container_id == "abc123def456"
        assert handle.runtime.workspace_path.path == "/workspace/proj"
        assert handle.runtime.profile.name == "docker-ci"

    def test_docker_persist_roundtrip(self, tmp_path: Path) -> None:
        from unittest.mock import MagicMock as _MM

        from tanren_core.adapters.remote_types import WorkspacePath
        from tanren_core.adapters.types import DockerEnvironmentRuntime

        conn = _MM()
        workspace = WorkspacePath(path="/workspace/proj", project="proj", branch="feat")
        original = EnvironmentHandle(
            env_id="env-rt",
            worktree_path=tmp_path / "ws",
            branch="feat",
            project="proj",
            runtime=DockerEnvironmentRuntime(
                container_id="deadbeef1234",
                connection=conn,
                workspace_path=workspace,
                profile=EnvironmentProfile(name="ci"),
                provision_start=50.0,
                workflow_id="wf-rt",
            ),
        )
        persisted = Worker._persist_handle(original, profile_name="ci")
        reconstructed = Worker._reconstruct_handle(persisted)

        assert reconstructed.env_id == original.env_id
        assert reconstructed.project == original.project
        assert reconstructed.runtime.kind == "docker"
        assert isinstance(reconstructed.runtime, DockerEnvironmentRuntime)
        assert reconstructed.runtime.container_id == "deadbeef1234"
