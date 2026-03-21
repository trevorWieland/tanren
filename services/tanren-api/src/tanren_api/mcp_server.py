"""MCP server — exposes tanren-api capabilities as MCP tools for LLM agents.

Tools are organized by domain: health, dispatch, VM, run, config, events.
Prefer ``dispatch_create`` for fire-and-forget jobs and ``run_full`` for
end-to-end lifecycle management.  Use the granular run_* tools only when
you need step-by-step control over provision → execute → teardown.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from fastmcp import FastMCP

from tanren_api.models import (
    ConfigResponse,
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    HealthResponse,
    MetricsCostsResponse,
    MetricsSummaryResponse,
    MetricsVMsResponse,
    PaginatedEvents,
    ReadinessResponse,
    RunEnvironment,
    RunExecuteAccepted,
    RunStatus,
    RunTeardownAccepted,
    VMDryRunResult,
    VMProvisionAccepted,
    VMProvisionStatus,
    VMReleaseConfirmed,
    VMSummary,
)
from tanren_core.schemas import AuthMode, Cli, Phase

if TYPE_CHECKING:
    from tanren_api.services import (
        ConfigService,
        DispatchService,
        EventsService,
        HealthService,
        MetricsService,
        RunService,
        VMService,
    )

mcp = FastMCP("tanren")


# ---------------------------------------------------------------------------
# Service accessors — set during lifespan via set_services()
# ---------------------------------------------------------------------------

_health_svc: HealthService | None = None
_dispatch_svc: DispatchService | None = None
_vm_svc: VMService | None = None
_run_svc: RunService | None = None
_config_svc: ConfigService | None = None
_events_svc: EventsService | None = None
_metrics_svc: MetricsService | None = None


def set_services(
    *,
    health: HealthService,
    dispatch: DispatchService,
    vm: VMService,
    run: RunService,
    config: ConfigService | None = None,
    events: EventsService,
    metrics: MetricsService | None = None,
) -> None:
    """Wire service instances into the MCP tool layer (called during app lifespan)."""
    global _health_svc, _dispatch_svc, _vm_svc, _run_svc, _config_svc, _events_svc, _metrics_svc
    _health_svc = health
    _dispatch_svc = dispatch
    _vm_svc = vm
    _run_svc = run
    _config_svc = config
    _events_svc = events
    _metrics_svc = metrics


# ---------------------------------------------------------------------------
# Health tools (no auth required)
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "Check whether the tanren API service is running and healthy. "
        "Returns service status, version, and uptime. No authentication required."
    ),
)
async def health_check() -> HealthResponse:
    """Check service health.

    Returns:
        HealthResponse with status, version, and uptime_seconds.
    """
    assert _health_svc is not None
    return await _health_svc.health()


@mcp.tool(
    description=(
        "Check whether the tanren API is ready to accept requests. "
        "Returns readiness status. No authentication required."
    ),
)
async def readiness_check() -> ReadinessResponse:
    """Check service readiness.

    Returns:
        ReadinessResponse with status field.
    """
    assert _health_svc is not None
    return await _health_svc.readiness()


# ---------------------------------------------------------------------------
# Dispatch tools
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "Submit a new dispatch — a request to run a coding agent against a "
        "project spec. Returns a dispatch_id for status polling via "
        "dispatch_get_status. The dispatch is executed asynchronously.\n\n"
        "Required fields: project, phase, branch, spec_folder, cli.\n"
        "Phases: do-task, review, gate, sweep.\n"
        "CLIs: claude, codex, opencode."
    ),
)
async def dispatch_create(
    project: str,
    phase: Phase,
    branch: str,
    spec_folder: str,
    cli: Cli,
    model: str | None = None,
    timeout: int = 1800,  # noqa: ASYNC109 — MCP tool param passed to Pydantic model, not asyncio
    context: str | None = None,
    gate_cmd: str | None = None,
    issue: str = "0",
) -> DispatchAccepted:
    """Create a new dispatch.

    Returns:
        DispatchAccepted with dispatch_id and status.
    """
    from tanren_api.models import DispatchRequest

    assert _dispatch_svc is not None
    body = DispatchRequest(
        project=project,
        phase=phase,
        branch=branch,
        spec_folder=spec_folder,
        cli=cli,
        model=model,
        timeout=timeout,
        context=context,
        gate_cmd=gate_cmd,
        issue=issue,
    )
    return await _dispatch_svc.create(body)


@mcp.tool(
    description=(
        "Get the current status of a dispatch by its workflow ID. "
        "Returns phase, project, status (pending/running/completed/failed/cancelled), "
        "outcome, and timestamps."
    ),
)
async def dispatch_get_status(dispatch_id: str) -> DispatchDetail:
    """Get dispatch status.

    Returns:
        DispatchDetail with full dispatch state including timestamps.
    """
    assert _dispatch_svc is not None
    return await _dispatch_svc.get(dispatch_id)


@mcp.tool(
    description=(
        "Cancel a pending or running dispatch. Returns confirmation or "
        "an error if the dispatch is already in a terminal state."
    ),
)
async def dispatch_cancel(dispatch_id: str) -> DispatchCancelled:
    """Cancel a dispatch.

    Returns:
        DispatchCancelled with dispatch_id and status.
    """
    assert _dispatch_svc is not None
    return await _dispatch_svc.cancel(dispatch_id)


# ---------------------------------------------------------------------------
# VM tools
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "List all active VM assignments. Returns vm_id, host, provider, "
        "associated workflow, project, and status for each VM."
    ),
)
async def vm_list() -> list[VMSummary]:
    """List active VMs.

    Returns:
        List of VMSummary records for each active VM assignment.
    """
    assert _vm_svc is not None
    return await _vm_svc.list_vms()


@mcp.tool(
    description=(
        "Start provisioning a new VM (non-blocking). Returns an env_id "
        "for polling via vm_provision_status. Use when you need a VM "
        "without immediately running a dispatch against it."
    ),
)
async def vm_provision(
    project: str,
    branch: str,
    environment_profile: str = "default",
) -> VMProvisionAccepted:
    """Provision a VM.

    Returns:
        VMProvisionAccepted with env_id and status.
    """
    from tanren_api.models import ProvisionRequest

    assert _vm_svc is not None
    body = ProvisionRequest(project=project, branch=branch, environment_profile=environment_profile)
    return await _vm_svc.provision(body)


@mcp.tool(
    description=(
        "Poll the status of a VM provisioning request. Returns status "
        "(provisioning/active/failed), vm_id, and host once ready."
    ),
)
async def vm_provision_status(env_id: str) -> VMProvisionStatus:
    """Get VM provision status.

    Returns:
        VMProvisionStatus with current status, vm_id, and host.
    """
    assert _vm_svc is not None
    return await _vm_svc.get_provision_status(env_id)


@mcp.tool(
    description="Release a VM by its vm_id. Destroys the VM and frees resources.",
)
async def vm_release(vm_id: str) -> VMReleaseConfirmed:
    """Release a VM.

    Returns:
        VMReleaseConfirmed with vm_id and status.
    """
    assert _vm_svc is not None
    return await _vm_svc.release(vm_id)


@mcp.tool(
    description=(
        "Dry-run a VM provision — shows what provider and server type would "
        "be used without actually creating resources. Useful for cost estimation."
    ),
)
async def vm_dry_run(
    project: str,
    branch: str,
    environment_profile: str = "default",
) -> VMDryRunResult:
    """Dry-run VM provision.

    Returns:
        VMDryRunResult with provider, server_type, and estimated_cost_hourly.
    """
    from tanren_api.models import ProvisionRequest

    assert _vm_svc is not None
    body = ProvisionRequest(project=project, branch=branch, environment_profile=environment_profile)
    return await _vm_svc.dry_run(body)


# ---------------------------------------------------------------------------
# Run tools — granular lifecycle management
#
# TIP: For most use cases, prefer ``run_full`` which handles the entire
# provision → execute → teardown lifecycle automatically. Use the individual
# run_provision / run_execute / run_teardown tools only when you need
# fine-grained control (e.g., running multiple phases on the same VM).
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "Provision a run environment (non-blocking). Returns env_id for "
        "polling via run_status. After provisioning completes, use "
        "run_execute to run phases against the environment, then "
        "run_teardown to release resources.\n\n"
        "TIP: For simple cases, use run_full instead — it handles "
        "provision + execute + teardown automatically."
    ),
)
async def run_provision(
    project: str,
    branch: str,
    environment_profile: str = "default",
) -> RunEnvironment:
    """Provision a run environment.

    Returns:
        RunEnvironment with env_id, vm_id, host, and status.
    """
    from tanren_api.models import ProvisionRequest

    assert _run_svc is not None
    body = ProvisionRequest(project=project, branch=branch, environment_profile=environment_profile)
    return await _run_svc.provision(body)


@mcp.tool(
    description=(
        "Execute a phase against an already-provisioned environment. "
        "The environment must be in 'provisioned' or 'completed' status. "
        "Returns a dispatch_id for tracking."
    ),
)
async def run_execute(
    env_id: str,
    project: str,
    spec_path: str,
    phase: Phase,
    cli: Cli,
    auth: AuthMode = AuthMode.API_KEY,
    model: str | None = None,
    timeout: int = 1800,  # noqa: ASYNC109 — MCP tool param passed to Pydantic model, not asyncio
    context: str | None = None,
    gate_cmd: str | None = None,
) -> RunExecuteAccepted:
    """Execute a phase on a provisioned environment.

    Returns:
        RunExecuteAccepted with env_id, dispatch_id, and status.
    """
    from tanren_api.models import ExecuteRequest

    assert _run_svc is not None
    body = ExecuteRequest(
        project=project,
        spec_path=spec_path,
        phase=phase,
        cli=cli,
        auth=auth,
        model=model,
        timeout=timeout,
        context=context,
        gate_cmd=gate_cmd,
    )
    return await _run_svc.execute(env_id, body)


@mcp.tool(
    description=(
        "Teardown a provisioned environment, releasing the backing VM. "
        "Call this after you are done executing phases. Safe to call "
        "in any environment state."
    ),
)
async def run_teardown(env_id: str) -> RunTeardownAccepted:
    """Teardown a run environment.

    Returns:
        RunTeardownAccepted with env_id and status.
    """
    assert _run_svc is not None
    return await _run_svc.teardown(env_id)


@mcp.tool(
    description=(
        "Run a full lifecycle: provision a VM, execute a phase, then "
        "teardown — all in one call. This is the recommended tool for "
        "most use cases. Returns a dispatch_id for status polling via "
        "dispatch_get_status.\n\n"
        "Required fields: project, branch, spec_path, phase, cli, auth."
    ),
)
async def run_full(
    project: str,
    branch: str,
    spec_path: str,
    phase: Phase,
    cli: Cli,
    auth: AuthMode = AuthMode.API_KEY,
    environment_profile: str = "default",
    timeout: int = 1800,  # noqa: ASYNC109 — MCP tool param passed to Pydantic model, not asyncio
    context: str | None = None,
    gate_cmd: str | None = None,
) -> DispatchAccepted:
    """Run full lifecycle (provision + execute + teardown).

    Returns:
        DispatchAccepted with dispatch_id and status.
    """
    from tanren_api.models import RunFullRequest

    assert _run_svc is not None
    body = RunFullRequest(
        project=project,
        branch=branch,
        spec_path=spec_path,
        phase=phase,
        cli=cli,
        auth=auth,
        environment_profile=environment_profile,
        timeout=timeout,
        context=context,
        gate_cmd=gate_cmd,
    )
    return await _run_svc.full(body)


@mcp.tool(
    description=(
        "Poll the status of a run environment. Returns status "
        "(provisioning/provisioned/executing/completed/failed), "
        "current phase, outcome, and duration."
    ),
)
async def run_status(env_id: str) -> RunStatus:
    """Get run environment status.

    Returns:
        RunStatus with env_id, status, phase, outcome, and duration.
    """
    assert _run_svc is not None
    return await _run_svc.status(env_id)


# ---------------------------------------------------------------------------
# Config tools
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "Get the current tanren configuration (non-secret fields only). "
        "Shows store backend, connection status, worker lanes, and version."
    ),
)
async def config_get() -> ConfigResponse | dict[str, str]:
    """Get non-secret configuration.

    Returns:
        ConfigResponse with configuration details, or error dict if unavailable.
    """
    if _config_svc is None:
        return {"error": "Configuration unavailable"}
    return await _config_svc.get()


# ---------------------------------------------------------------------------
# Events tools
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "Query structured events with optional filters. Returns paginated "
        "typed event records (dispatch received, phase started/completed, "
        "VM provisioned/released, errors, retries, token usage, etc.)."
    ),
)
async def events_query(
    workflow_id: str | None = None,
    event_type: str | None = None,
    limit: int = 50,
    offset: int = 0,
) -> PaginatedEvents:
    """Query events.

    Returns:
        PaginatedEvents with events list, total count, and pagination info.
    """
    assert _events_svc is not None
    limit = max(1, min(limit, 100))
    offset = max(0, offset)
    return await _events_svc.query(
        workflow_id=workflow_id,
        event_type=event_type,
        limit=limit,
        offset=offset,
    )


# ---------------------------------------------------------------------------
# Metrics tools
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "Get workflow summary metrics: success/failure rate, duration stats. "
        "Optionally filter by time range (since/until as ISO 8601) and project."
    ),
)
async def metrics_summary(
    since: str | None = None,
    until: str | None = None,
    project: str | None = None,
) -> MetricsSummaryResponse:
    """Get summary metrics.

    Returns:
        MetricsSummaryResponse with success rate, duration percentiles, and counts.
    """
    assert _metrics_svc is not None
    return await _metrics_svc.summary(since=since, until=until, project=project)


@mcp.tool(
    description=(
        "Get token cost metrics grouped by model, day, or workflow. "
        "group_by: 'model' (default), 'day', or 'workflow'."
    ),
)
async def metrics_costs(
    since: str | None = None,
    until: str | None = None,
    project: str | None = None,
    group_by: str = "model",
) -> MetricsCostsResponse | dict[str, str]:
    """Get cost metrics.

    Returns:
        MetricsCostsResponse with cost buckets, or error dict for invalid group_by.
    """
    assert _metrics_svc is not None
    valid = {"model", "day", "workflow"}
    if group_by not in valid:
        return {"error": f"Invalid group_by '{group_by}'. Must be: model, day, workflow"}
    return await _metrics_svc.costs(since=since, until=until, project=project, group_by=group_by)


@mcp.tool(
    description=(
        "Get VM utilization metrics: provisioned, released, active counts, "
        "duration, and estimated cost."
    ),
)
async def metrics_vms(
    since: str | None = None,
    until: str | None = None,
    project: str | None = None,
) -> MetricsVMsResponse:
    """Get VM metrics.

    Returns:
        MetricsVMsResponse with VM counts, duration, and estimated cost.
    """
    assert _metrics_svc is not None
    return await _metrics_svc.vms(since=since, until=until, project=project)
