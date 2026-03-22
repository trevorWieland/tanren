"""VM service — list, provision, poll, release, and dry-run VMs via store."""

from __future__ import annotations

import logging
import uuid
from datetime import UTC, datetime

from tanren_api.errors import NotFoundError
from tanren_api.models import (
    ProvisionRequest,
    VMProvisionAccepted,
    VMProvisionStatus,
    VMReleaseConfirmed,
    VMStatus,
    VMSummary,
)
from tanren_core.schemas import Cli, Dispatch, Phase
from tanren_core.store.enums import DispatchMode, StepStatus, StepType, cli_to_lane
from tanren_core.store.events import DispatchCreated
from tanren_core.store.payloads import (
    DryRunStepPayload,
    ProvisionResult,
    ProvisionStepPayload,
    TeardownStepPayload,
)
from tanren_core.store.protocols import EventStore, JobQueue, StateStore

logger = logging.getLogger(__name__)


def _now() -> str:
    return datetime.now(UTC).isoformat()


class VMService:
    """VM lifecycle management via store protocols."""

    def __init__(
        self,
        *,
        event_store: EventStore,
        job_queue: JobQueue,
        state_store: StateStore,
    ) -> None:
        """Initialize with store dependencies only."""
        self._event_store = event_store
        self._job_queue = job_queue
        self._state_store = state_store

    async def list_vms(self) -> list[VMSummary]:
        """List active VMs by scanning provision steps without matching teardowns.

        Returns:
            list[VMSummary]: Active VM summaries.
        """
        from tanren_core.store.views import DispatchListFilter

        dispatches = await self._state_store.query_dispatches(DispatchListFilter(limit=200))
        vms: list[VMSummary] = []
        for d in dispatches:
            steps = await self._state_store.get_steps_for_dispatch(d.dispatch_id)
            prov = next(
                (
                    s
                    for s in steps
                    if s.step_type == StepType.PROVISION and s.status == StepStatus.COMPLETED
                ),
                None,
            )
            td = next(
                (
                    s
                    for s in steps
                    if s.step_type == StepType.TEARDOWN and s.status == StepStatus.COMPLETED
                ),
                None,
            )
            if prov and prov.result_json and not td:
                result = ProvisionResult.model_validate_json(prov.result_json)
                handle = result.handle
                if handle.vm:
                    vms.append(
                        VMSummary(
                            vm_id=handle.vm.vm_id,
                            host=handle.vm.host,
                            provider=handle.vm.provider,
                            workflow_id=d.dispatch_id,
                            project=d.dispatch.project,
                            status=VMStatus.ACTIVE,
                            created_at=handle.vm.created_at,
                        )
                    )
        return vms

    async def provision(self, body: ProvisionRequest) -> VMProvisionAccepted:
        """Enqueue a provision step for a new VM.

        Returns:
            VMProvisionAccepted with env_id for polling.
        """
        workflow_id = f"vm-provision-{body.project}-{uuid.uuid4().hex[:8]}"

        dispatch = Dispatch(
            workflow_id=workflow_id,
            project=body.project,
            phase=Phase.DO_TASK,
            branch=body.branch,
            spec_folder="",
            cli=Cli.CLAUDE,
            timeout=1800,
            environment_profile=body.resolved_profile.name,
            resolved_profile=body.resolved_profile,
        )

        lane = cli_to_lane(dispatch.cli)
        await self._state_store.create_dispatch_projection(
            dispatch_id=workflow_id,
            mode=DispatchMode.MANUAL,
            lane=lane,
            preserve_on_failure=True,
            dispatch_json=dispatch.model_dump_json(),
        )
        await self._event_store.append(
            DispatchCreated(
                timestamp=_now(),
                workflow_id=workflow_id,
                dispatch=dispatch,
                mode=DispatchMode.MANUAL,
                lane=lane,
            )
        )

        step_id = uuid.uuid4().hex
        payload = ProvisionStepPayload(dispatch=dispatch)
        await self._job_queue.enqueue_step(
            step_id=step_id,
            dispatch_id=workflow_id,
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        return VMProvisionAccepted(env_id=workflow_id)

    async def get_provision_status(self, env_id: str) -> VMProvisionStatus:
        """Poll provision status.

        Returns:
            VMProvisionStatus with current state.

        Raises:
            NotFoundError: If not found.
        """
        view = await self._state_store.get_dispatch(env_id)
        if view is None:
            raise NotFoundError(f"Provision {env_id} not found")

        steps = await self._state_store.get_steps_for_dispatch(env_id)
        prov = next((s for s in steps if s.step_type == StepType.PROVISION), None)

        if prov and prov.status == StepStatus.COMPLETED and prov.result_json:
            result = ProvisionResult.model_validate_json(prov.result_json)
            handle = result.handle
            return VMProvisionStatus(
                env_id=env_id,
                status=VMStatus.ACTIVE,
                vm_id=handle.vm.vm_id if handle.vm else None,
                host=handle.vm.host if handle.vm else None,
                provider=handle.vm.provider if handle.vm else None,
                created_at=view.created_at,
            )
        elif prov and prov.status == StepStatus.FAILED:
            return VMProvisionStatus(env_id=env_id, status=VMStatus.FAILED)
        else:
            return VMProvisionStatus(env_id=env_id, status=VMStatus.PROVISIONING)

    async def release(self, vm_id: str) -> VMReleaseConfirmed:
        """Enqueue a teardown step for the VM.

        Returns:
            VMReleaseConfirmed.

        Raises:
            NotFoundError: If VM not found.
        """
        # Find the dispatch that provisioned this VM
        from tanren_core.store.views import DispatchListFilter

        dispatches = await self._state_store.query_dispatches(DispatchListFilter(limit=200))
        for d in dispatches:
            steps = await self._state_store.get_steps_for_dispatch(d.dispatch_id)
            prov = next(
                (
                    s
                    for s in steps
                    if s.step_type == StepType.PROVISION
                    and s.status == StepStatus.COMPLETED
                    and s.result_json
                ),
                None,
            )
            if prov and prov.result_json is not None and vm_id in prov.result_json:
                result = ProvisionResult.model_validate_json(prov.result_json)
                step_id = uuid.uuid4().hex
                payload = TeardownStepPayload(dispatch=d.dispatch, handle=result.handle)
                await self._job_queue.enqueue_step(
                    step_id=step_id,
                    dispatch_id=d.dispatch_id,
                    step_type="teardown",
                    step_sequence=2,
                    lane=None,
                    payload_json=payload.model_dump_json(),
                )
                return VMReleaseConfirmed(vm_id=vm_id)

        raise NotFoundError(f"VM {vm_id} not found")

    async def dry_run(self, body: ProvisionRequest) -> VMProvisionAccepted:
        """Enqueue a DRY_RUN step and return the dispatch_id for polling.

        Returns:
            VMProvisionAccepted with env_id for polling.
        """
        workflow_id = f"vm-dryrun-{body.project}-{uuid.uuid4().hex[:8]}"

        dispatch = Dispatch(
            workflow_id=workflow_id,
            project=body.project,
            phase=Phase.DO_TASK,
            branch=body.branch,
            spec_folder="",
            cli=Cli.CLAUDE,
            timeout=60,
            environment_profile=body.resolved_profile.name,
            resolved_profile=body.resolved_profile,
        )

        lane = cli_to_lane(dispatch.cli)
        await self._state_store.create_dispatch_projection(
            dispatch_id=workflow_id,
            mode=DispatchMode.MANUAL,
            lane=lane,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        await self._event_store.append(
            DispatchCreated(
                timestamp=_now(),
                workflow_id=workflow_id,
                dispatch=dispatch,
                mode=DispatchMode.MANUAL,
                lane=lane,
            )
        )

        step_id = uuid.uuid4().hex
        payload = DryRunStepPayload(dispatch=dispatch)
        await self._job_queue.enqueue_step(
            step_id=step_id,
            dispatch_id=workflow_id,
            step_type="dry_run",
            step_sequence=0,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        return VMProvisionAccepted(env_id=workflow_id)
