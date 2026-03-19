"""MCP server — exposes tanren-api capabilities as MCP tools for LLM agents.

Tools are organized by domain: health, dispatch, VM, run, config, events.
Prefer ``dispatch_create`` for fire-and-forget jobs and ``run_full`` for
end-to-end lifecycle management.  Use the granular run_* tools only when
you need step-by-step control over provision → execute → teardown.
"""
# ruff: noqa: DOC201,ANN401,ASYNC109

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from fastmcp import FastMCP

if TYPE_CHECKING:
    from tanren_api.services import (
        ConfigService,
        DispatchService,
        EventsService,
        HealthService,
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


def set_services(
    *,
    health: HealthService,
    dispatch: DispatchService,
    vm: VMService,
    run: RunService,
    config: ConfigService | None = None,
    events: EventsService,
) -> None:
    """Wire service instances into the MCP tool layer (called during app lifespan)."""
    global _health_svc, _dispatch_svc, _vm_svc, _run_svc, _config_svc, _events_svc
    _health_svc = health
    _dispatch_svc = dispatch
    _vm_svc = vm
    _run_svc = run
    _config_svc = config
    _events_svc = events


def _model_dump(obj: Any) -> dict[str, Any]:
    """Serialize a Pydantic model to dict (JSON-safe)."""
    return obj.model_dump(mode="json")


# ---------------------------------------------------------------------------
# Health tools (no auth required)
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "Check whether the tanren API service is running and healthy. "
        "Returns service status, version, and uptime. No authentication required."
    ),
)
async def health_check() -> dict[str, Any]:
    """Check service health."""
    assert _health_svc is not None
    return _model_dump(await _health_svc.health())


@mcp.tool(
    description=(
        "Check whether the tanren API is ready to accept requests. "
        "Returns readiness status. No authentication required."
    ),
)
async def readiness_check() -> dict[str, Any]:
    """Check service readiness."""
    assert _health_svc is not None
    return _model_dump(await _health_svc.readiness())


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
    phase: str,
    branch: str,
    spec_folder: str,
    cli: str,
    model: str | None = None,
    timeout: int = 1800,
    context: str | None = None,
    gate_cmd: str | None = None,
    issue: str = "0",
) -> dict[str, Any]:
    """Create a new dispatch."""
    from tanren_api.models import DispatchRequest  # noqa: PLC0415

    assert _dispatch_svc is not None
    body = DispatchRequest(
        project=project,
        phase=phase,  # type: ignore[arg-type]
        branch=branch,
        spec_folder=spec_folder,
        cli=cli,  # type: ignore[arg-type]
        model=model,
        timeout=timeout,
        context=context,
        gate_cmd=gate_cmd,
        issue=issue,
    )
    return _model_dump(await _dispatch_svc.create(body))


@mcp.tool(
    description=(
        "Get the current status of a dispatch by its workflow ID. "
        "Returns phase, project, status (pending/running/completed/failed/cancelled), "
        "outcome, and timestamps."
    ),
)
async def dispatch_get_status(dispatch_id: str) -> dict[str, Any]:
    """Get dispatch status."""
    assert _dispatch_svc is not None
    return _model_dump(await _dispatch_svc.get(dispatch_id))


@mcp.tool(
    description=(
        "Cancel a pending or running dispatch. Returns confirmation or "
        "an error if the dispatch is already in a terminal state."
    ),
)
async def dispatch_cancel(dispatch_id: str) -> dict[str, Any]:
    """Cancel a dispatch."""
    assert _dispatch_svc is not None
    return _model_dump(await _dispatch_svc.cancel(dispatch_id))


# ---------------------------------------------------------------------------
# VM tools
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "List all active VM assignments. Returns vm_id, host, provider, "
        "associated workflow, project, and status for each VM."
    ),
)
async def vm_list() -> list[dict[str, Any]]:
    """List active VMs."""
    assert _vm_svc is not None
    return [_model_dump(vm) for vm in await _vm_svc.list_vms()]


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
) -> dict[str, Any]:
    """Provision a VM."""
    from tanren_api.models import ProvisionRequest  # noqa: PLC0415

    assert _vm_svc is not None
    body = ProvisionRequest(project=project, branch=branch, environment_profile=environment_profile)
    return _model_dump(await _vm_svc.provision(body))


@mcp.tool(
    description=(
        "Poll the status of a VM provisioning request. Returns status "
        "(provisioning/active/failed), vm_id, and host once ready."
    ),
)
async def vm_provision_status(env_id: str) -> dict[str, Any]:
    """Get VM provision status."""
    assert _vm_svc is not None
    return _model_dump(await _vm_svc.get_provision_status(env_id))


@mcp.tool(
    description="Release a VM by its vm_id. Destroys the VM and frees resources.",
)
async def vm_release(vm_id: str) -> dict[str, Any]:
    """Release a VM."""
    assert _vm_svc is not None
    return _model_dump(await _vm_svc.release(vm_id))


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
) -> dict[str, Any]:
    """Dry-run VM provision."""
    from tanren_api.models import ProvisionRequest  # noqa: PLC0415

    assert _vm_svc is not None
    body = ProvisionRequest(project=project, branch=branch, environment_profile=environment_profile)
    return _model_dump(await _vm_svc.dry_run(body))


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
) -> dict[str, Any]:
    """Provision a run environment."""
    from tanren_api.models import ProvisionRequest  # noqa: PLC0415

    assert _run_svc is not None
    body = ProvisionRequest(project=project, branch=branch, environment_profile=environment_profile)
    return _model_dump(await _run_svc.provision(body))


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
    phase: str,
    cli: str,
    auth: str = "api_key",
    model: str | None = None,
    timeout: int = 1800,
    context: str | None = None,
    gate_cmd: str | None = None,
) -> dict[str, Any]:
    """Execute a phase on a provisioned environment."""
    from tanren_api.models import ExecuteRequest  # noqa: PLC0415

    assert _run_svc is not None
    body = ExecuteRequest(
        project=project,
        spec_path=spec_path,
        phase=phase,  # type: ignore[arg-type]
        cli=cli,  # type: ignore[arg-type]
        auth=auth,  # type: ignore[arg-type]
        model=model,
        timeout=timeout,
        context=context,
        gate_cmd=gate_cmd,
    )
    return _model_dump(await _run_svc.execute(env_id, body))


@mcp.tool(
    description=(
        "Teardown a provisioned environment, releasing the backing VM. "
        "Call this after you are done executing phases. Safe to call "
        "in any environment state."
    ),
)
async def run_teardown(env_id: str) -> dict[str, Any]:
    """Teardown a run environment."""
    assert _run_svc is not None
    return _model_dump(await _run_svc.teardown(env_id))


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
    phase: str,
    cli: str,
    auth: str = "api_key",
    environment_profile: str = "default",
    timeout: int = 1800,
    context: str | None = None,
    gate_cmd: str | None = None,
) -> dict[str, Any]:
    """Run full lifecycle (provision + execute + teardown)."""
    from tanren_api.models import RunFullRequest  # noqa: PLC0415

    assert _run_svc is not None
    body = RunFullRequest(
        project=project,
        branch=branch,
        spec_path=spec_path,
        phase=phase,  # type: ignore[arg-type]
        cli=cli,  # type: ignore[arg-type]
        auth=auth,  # type: ignore[arg-type]
        environment_profile=environment_profile,
        timeout=timeout,
        context=context,
        gate_cmd=gate_cmd,
    )
    return _model_dump(await _run_svc.full(body))


@mcp.tool(
    description=(
        "Poll the status of a run environment. Returns status "
        "(provisioning/provisioned/executing/completed/failed), "
        "current phase, outcome, and duration."
    ),
)
async def run_status(env_id: str) -> dict[str, Any]:
    """Get run environment status."""
    assert _run_svc is not None
    return _model_dump(await _run_svc.status(env_id))


# ---------------------------------------------------------------------------
# Config tools
# ---------------------------------------------------------------------------


@mcp.tool(
    description=(
        "Get the current tanren configuration (non-secret fields only). "
        "Shows IPC directory, poll intervals, concurrency limits, and "
        "whether events and remote execution are enabled."
    ),
)
async def config_get() -> dict[str, Any]:
    """Get non-secret configuration."""
    if _config_svc is None:
        return {"error": "Configuration unavailable — WM_* environment variables not set"}
    return _model_dump(await _config_svc.get())


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
) -> dict[str, Any]:
    """Query events."""
    assert _events_svc is not None
    limit = max(1, min(limit, 100))
    offset = max(0, offset)
    result = await _events_svc.query(
        workflow_id=workflow_id,
        event_type=event_type,
        limit=limit,
        offset=offset,
    )
    return _model_dump(result)
