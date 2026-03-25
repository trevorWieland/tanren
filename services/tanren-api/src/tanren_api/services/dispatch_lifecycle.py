"""Shared dispatch creation logic used by both DispatchService and RunService."""

from __future__ import annotations

import time
import uuid
from datetime import UTC, datetime
from typing import TYPE_CHECKING

from tanren_api.models import DispatchAccepted, DispatchRequest
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli, Dispatch, Phase
from tanren_core.store.enums import DispatchMode, cli_to_lane
from tanren_core.store.events import DispatchCreated
from tanren_core.store.payloads import ProvisionStepPayload
from tanren_core.store.protocols import EventStore, JobQueue, StateStore

if TYPE_CHECKING:
    from tanren_core.worker_config import WorkerConfig


def _resolve_cli_auth(
    body: DispatchRequest,
    config: WorkerConfig | None = None,
) -> tuple[Cli, AuthMode, str | None]:
    """Resolve cli/auth/model from roles.yml when not explicitly provided.

    Args:
        body: The dispatch request.
        config: WorkerConfig for agent tool resolution. Required when cli is None.

    Returns:
        Tuple of (cli, auth, model).
    """
    cli = body.cli
    auth = body.auth
    model = body.model
    if cli is None:
        if body.phase == Phase.GATE:
            # Gate always uses bash — no roles.yml lookup needed
            cli = Cli.BASH
            auth = auth or AuthMode.API_KEY
        else:
            if config is None:
                raise RuntimeError("WorkerConfig required for CLI auto-resolution")
            from tanren_core.dispatch_resolver import resolve_agent_tool

            tool = resolve_agent_tool(config, body.phase)
            cli = tool.cli
            auth = auth or tool.auth
            model = model or tool.model
    if auth is None:
        auth = AuthMode.API_KEY
    return cli, auth, model


async def create_dispatch_from_request(
    *,
    body: DispatchRequest,
    event_store: EventStore,
    job_queue: JobQueue,
    state_store: StateStore,
    config: WorkerConfig | None = None,
) -> DispatchAccepted:
    """Create a dispatch by appending events and enqueuing the first step.

    This is the core dispatch-creation logic shared by DispatchService.create()
    and RunService.full().

    NOTE: These three operations (create_dispatch_projection, append
    DispatchCreated, enqueue_step) each run in separate transactions.
    Making this atomic requires a store protocol method that bundles
    all three — deferred to future store redesign.  Partial failures
    are detectable (dispatch exists with no steps).
    """
    epoch = time.time_ns()
    issue = body.issue if body.issue != "0" else str(epoch)
    workflow_id = f"wf-{body.project}-{issue}-{epoch}"

    cli, auth, model = _resolve_cli_auth(body, config)

    # Resolve gate_cmd from profile defaults when not explicitly provided
    gate_cmd = body.gate_cmd
    if body.phase == Phase.GATE and not gate_cmd and config is not None:
        from tanren_core.dispatch_resolver import resolve_gate_cmd

        gate_cmd = resolve_gate_cmd(
            config, body.project, body.resolved_profile.name, body.phase, gate_cmd
        )

    dispatch = Dispatch(
        workflow_id=workflow_id,
        project=body.project,
        phase=body.phase,
        branch=body.branch,
        spec_folder=body.spec_folder,
        cli=cli,
        auth=auth,
        model=model,
        timeout=body.timeout,
        environment_profile=body.resolved_profile.name,
        context=body.context,
        gate_cmd=gate_cmd,
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
