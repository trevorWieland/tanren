"""Dispatch builder — single source of truth for resolving dispatch inputs.

Consolidates profile, env, secret, CLI/auth, and gate-command resolution
into one module.  All entry points (CLI, MCP, REST) call into here instead
of duplicating resolution logic.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

from tanren_core.dispatch_resolver import (
    resolve_agent_tool,
    resolve_cloud_secrets_from_config,
    resolve_remote_config,
    resolve_required_secrets,
)
from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    EnvironmentProfileType,
    parse_environment_profiles,
)
from tanren_core.env.gates import resolve_gate_cmd as _resolve_gate_cmd_from_profile
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli, Phase

if TYPE_CHECKING:
    from tanren_core.config_resolver import ConfigResolver
    from tanren_core.worker_config import WorkerConfig


@dataclass(frozen=True)
class ResolvedInputs:
    """Fully resolved configuration for dispatch/provision creation.

    Produced by the builder functions; consumed by all entry points
    (CLI, MCP, REST) to construct request/dispatch objects.
    """

    profile: EnvironmentProfile
    project_env: dict[str, str] = field(default_factory=dict)
    cloud_secrets: dict[str, str] = field(default_factory=dict)
    required_secrets: tuple[str, ...] = ()
    cli: Cli = Cli.CLAUDE
    auth: AuthMode = AuthMode.API_KEY
    model: str | None = None
    gate_cmd: str | None = None


# ── Public builder functions ─────────────────────────────────────────────


async def resolve_dispatch_inputs(
    *,
    resolver: ConfigResolver,
    config: WorkerConfig,
    project: str,
    phase: Phase,
    branch: str = "main",
    environment_profile: str = "default",
    # Optional overrides — bypass resolution when pre-resolved
    cli: Cli | None = None,
    auth: AuthMode | None = None,
    model: str | None = None,
    gate_cmd: str | None = None,
    resolved_profile: EnvironmentProfile | None = None,
    project_env: dict[str, str] | None = None,
    cloud_secrets: dict[str, str] | None = None,
) -> ResolvedInputs:
    """Resolve all inputs needed to create a dispatch.

    Handles profile, env, secrets, CLI/auth/model, and gate command
    resolution.  Each input can be pre-resolved by the caller; if
    ``None``, the builder resolves it via the ``resolver``.

    Returns:
        ResolvedInputs with all fields populated.

    Raises:
        ValueError: If gate command is empty or profile not found.
    """
    profile, p_env, c_secrets, req_secrets, _tanren_data = await _resolve_common(
        resolver=resolver,
        config=config,
        project=project,
        branch=branch,
        environment_profile=environment_profile,
        resolved_profile=resolved_profile,
        project_env=project_env,
        cloud_secrets=cloud_secrets,
    )

    # CLI/auth/model resolution
    resolved_cli, resolved_auth, resolved_model = resolve_cli_auth(
        config=config, phase=phase, cli=cli, auth=auth, model=model
    )

    # Gate command resolution
    resolved_gate_cmd = gate_cmd
    if phase == Phase.GATE and not resolved_gate_cmd:
        resolved_gate_cmd = _resolve_gate_cmd_from_profile(profile, phase)
        normalized = resolved_gate_cmd.strip() if resolved_gate_cmd else ""
        if not normalized:
            msg = (
                "Gate phase requires a non-empty gate command. "
                "Provide gate_cmd or configure environment.<profile>.gate_cmd in tanren.yml."
            )
            raise ValueError(msg)
        resolved_gate_cmd = normalized

    return ResolvedInputs(
        profile=profile,
        project_env=p_env,
        cloud_secrets=c_secrets,
        required_secrets=req_secrets,
        cli=resolved_cli,
        auth=resolved_auth,
        model=resolved_model,
        gate_cmd=resolved_gate_cmd,
    )


async def resolve_provision_inputs(
    *,
    resolver: ConfigResolver,
    config: WorkerConfig,
    project: str,
    branch: str = "main",
    environment_profile: str = "default",
    resolved_profile: EnvironmentProfile | None = None,
    project_env: dict[str, str] | None = None,
    cloud_secrets: dict[str, str] | None = None,
) -> ResolvedInputs:
    """Resolve inputs needed for a provision-only request (no CLI/auth/gate).

    Returns:
        ResolvedInputs with profile, env, secrets; CLI defaults to CLAUDE.
    """
    profile, p_env, c_secrets, req_secrets, _tanren_data = await _resolve_common(
        resolver=resolver,
        config=config,
        project=project,
        branch=branch,
        environment_profile=environment_profile,
        resolved_profile=resolved_profile,
        project_env=project_env,
        cloud_secrets=cloud_secrets,
    )

    return ResolvedInputs(
        profile=profile,
        project_env=p_env,
        cloud_secrets=c_secrets,
        required_secrets=req_secrets,
        cli=Cli.CLAUDE,
        auth=AuthMode.API_KEY,
    )


# ── Internal helpers ─────────────────────────────────────────────────────


async def _resolve_common(
    *,
    resolver: ConfigResolver,
    config: WorkerConfig,
    project: str,
    branch: str,
    environment_profile: str,
    resolved_profile: EnvironmentProfile | None,
    project_env: dict[str, str] | None,
    cloud_secrets: dict[str, str] | None,
) -> tuple[EnvironmentProfile, dict[str, str], dict[str, str], tuple[str, ...], dict]:
    """Shared resolution for profile, env, secrets.

    Returns:
        (profile, project_env, cloud_secrets, required_secrets, tanren_config_data)

    Raises:
        ValueError: If the requested environment profile is not found.
    """
    tanren_data: dict = {}

    # 1. Profile resolution
    if resolved_profile is not None:
        profile = resolved_profile
    else:
        tanren_data = await resolver.load_tanren_config(project, branch)
        profiles = parse_environment_profiles(tanren_data)
        profile = profiles.get(environment_profile)
        if profile is None:
            available = sorted(profiles.keys())
            msg = (
                f"Environment profile '{environment_profile}' not found in tanren.yml. "
                f"Available: {available}"
            )
            raise ValueError(msg)

        # Enrich REMOTE profiles with remote_config from remote.yml
        if profile.type == EnvironmentProfileType.REMOTE and profile.remote_config is None:
            remote_cfg = resolve_remote_config(config, project)
            profile = profile.model_copy(update={"remote_config": remote_cfg})

    # 2. Project env
    if project_env is not None:
        p_env = project_env
    else:
        p_env = await resolver.load_project_env(project)

    # 3. Cloud secrets
    if cloud_secrets is not None:
        c_secrets = cloud_secrets
    else:
        if not tanren_data:
            tanren_data = await resolver.load_tanren_config(project, branch)
        c_secrets = await resolve_cloud_secrets_from_config(tanren_data)

    # 4. Required secrets (always computed from profile)
    req_secrets = resolve_required_secrets(profile)

    return profile, p_env, c_secrets, req_secrets, tanren_data


def resolve_cli_auth(
    *,
    config: WorkerConfig,
    phase: Phase,
    cli: Cli | None,
    auth: AuthMode | None,
    model: str | None,
) -> tuple[Cli, AuthMode, str | None]:
    """Resolve CLI, auth mode, and model from roles.yml if not provided.

    Returns:
        Tuple of (cli, auth_mode, model).
    """
    resolved_cli = cli
    resolved_auth = auth
    resolved_model = model

    if resolved_cli is None:
        if phase == Phase.GATE:
            resolved_cli = Cli.BASH
            resolved_auth = resolved_auth or AuthMode.API_KEY
        else:
            tool = resolve_agent_tool(config, phase)
            resolved_cli = tool.cli
            resolved_auth = resolved_auth or tool.auth
            resolved_model = resolved_model or tool.model

    if resolved_auth is None:
        resolved_auth = AuthMode.API_KEY

    return resolved_cli, resolved_auth, resolved_model
