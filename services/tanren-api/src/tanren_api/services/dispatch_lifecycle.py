"""Shared dispatch creation logic used by both DispatchService and RunService."""

from __future__ import annotations

import time
import uuid
from datetime import UTC, datetime

from tanren_api.models import DispatchAccepted, DispatchRequest
from tanren_core.schemas import Dispatch
from tanren_core.store.enums import DispatchMode, cli_to_lane
from tanren_core.store.events import DispatchCreated
from tanren_core.store.payloads import ProvisionStepPayload
from tanren_core.store.protocols import EventStore, JobQueue, StateStore


async def create_dispatch_from_request(
    *,
    body: DispatchRequest,
    event_store: EventStore,
    job_queue: JobQueue,
    state_store: StateStore,
) -> DispatchAccepted:
    """Create a dispatch by appending events and enqueuing the first step.

    This is the core dispatch-creation logic shared by DispatchService.create()
    and RunService.full().
    """
    epoch = time.time_ns()
    issue = body.issue if body.issue != "0" else str(epoch)
    workflow_id = f"wf-{body.project}-{issue}-{epoch}"

    dispatch = Dispatch(
        workflow_id=workflow_id,
        project=body.project,
        phase=body.phase,
        branch=body.branch,
        spec_folder=body.spec_folder,
        cli=body.cli,
        auth=body.auth,
        model=body.model,
        timeout=body.timeout,
        environment_profile=body.resolved_profile.name,
        context=body.context,
        gate_cmd=body.gate_cmd,
        resolved_profile=body.resolved_profile,
        preserve_on_failure=body.preserve_on_failure,
        project_env=body.project_env,
        cloud_secrets=body.cloud_secrets,
    )

    lane = cli_to_lane(body.cli)

    await state_store.create_dispatch_projection(
        dispatch_id=workflow_id,
        mode=DispatchMode.AUTO,
        lane=lane,
        preserve_on_failure=dispatch.preserve_on_failure,
        dispatch_json=dispatch.model_dump_json(),
    )

    await event_store.append(
        DispatchCreated(
            timestamp=datetime.now(UTC).isoformat().replace("+00:00", "Z"),
            workflow_id=workflow_id,
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=lane,
        )
    )

    step_id = uuid.uuid4().hex
    payload = ProvisionStepPayload(dispatch=dispatch)
    await job_queue.enqueue_step(
        step_id=step_id,
        dispatch_id=workflow_id,
        step_type="provision",
        step_sequence=0,
        lane=None,
        payload_json=payload.model_dump_json(),
    )

    return DispatchAccepted(dispatch_id=workflow_id)
