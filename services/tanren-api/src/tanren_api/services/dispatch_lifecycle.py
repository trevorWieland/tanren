"""Shared dispatch creation logic used by both DispatchService and RunService."""

from __future__ import annotations

import time
import uuid
from datetime import UTC, datetime

from tanren_api.models import DispatchAccepted, DispatchRequest
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli, Dispatch, Phase
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
    user_id: str = "",
) -> DispatchAccepted:
    """Create a dispatch by appending events and enqueuing the first step.

    Expects a fully-resolved ``DispatchRequest`` — cli, auth, model, and
    gate_cmd must already be set (by the dispatch builder or the caller).
    Profile, project_env, cloud_secrets, and required_secrets must also
    be pre-resolved.

    NOTE: These three operations (create_dispatch_projection, append
    DispatchCreated, enqueue_step) each run in separate transactions.
    Making this atomic requires a store protocol method that bundles
    all three — deferred to future store redesign.  Partial failures
    are detectable (dispatch exists with no steps).
    """
    epoch = time.time_ns()
    issue = body.issue if body.issue != "0" else str(epoch)
    workflow_id = f"wf-{body.project}-{issue}-{epoch}"

    # Fallback CLI/auth resolution for REST API callers that don't use the builder
    cli = body.cli
    auth = body.auth
    if cli is None:
        cli = Cli.BASH if body.phase == Phase.GATE else Cli.CLAUDE
    if auth is None:
        auth = AuthMode.API_KEY

    dispatch = Dispatch(
        workflow_id=workflow_id,
        project=body.project,
        phase=body.phase,
        branch=body.branch,
        spec_folder=body.spec_folder,
        cli=cli,
        auth=auth,
        model=body.model,
        timeout=body.timeout,
        environment_profile=body.resolved_profile.name,
        context=body.context,
        gate_cmd=body.gate_cmd,
        resolved_profile=body.resolved_profile,
        preserve_on_failure=body.preserve_on_failure,
        project_env=body.project_env,
        cloud_secrets=body.cloud_secrets,
        required_secrets=body.required_secrets,
    )

    lane = cli_to_lane(cli)

    await state_store.create_dispatch_projection(
        dispatch_id=workflow_id,
        mode=DispatchMode.AUTO,
        lane=lane,
        preserve_on_failure=dispatch.preserve_on_failure,
        dispatch_json=dispatch.model_dump_json(),
        user_id=user_id,
    )

    await event_store.append(
        DispatchCreated(
            timestamp=datetime.now(UTC).isoformat().replace("+00:00", "Z"),
            entity_id=workflow_id,
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
