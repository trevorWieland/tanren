"""Shared dispatch creation logic used by both DispatchService and RunService.

This is a thin adapter that maps ``DispatchRequest`` (API model) to
``Dispatch`` (core schema) and delegates to the ``dispatch_orchestrator``
in ``tanren_core``.
"""

from __future__ import annotations

import time

from tanren_api.models import DispatchAccepted, DispatchRequest
from tanren_core.dispatch_orchestrator import create_dispatch
from tanren_core.schemas import Dispatch
from tanren_core.store.enums import DispatchMode
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
    """
    dispatch = _to_dispatch(body)

    result = await create_dispatch(
        dispatch=dispatch,
        mode=DispatchMode.AUTO,
        event_store=event_store,
        job_queue=job_queue,
        state_store=state_store,
        user_id=user_id,
    )

    return DispatchAccepted(dispatch_id=result.dispatch_id)


def _to_dispatch(body: DispatchRequest) -> Dispatch:
    """Map an API ``DispatchRequest`` to a core ``Dispatch`` model.

    All fields must be pre-resolved — cli, auth, gate_cmd are required
    by the DispatchRequest schema (enforced via Pydantic validation).
    """
    epoch = time.time_ns()
    issue = body.issue if body.issue != "0" else str(epoch)
    workflow_id = f"wf-{body.project}-{issue}-{epoch}"

    return Dispatch(
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
        required_secrets=body.required_secrets,
    )
