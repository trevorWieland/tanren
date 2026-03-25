"""Dispatch resolution — resolve profiles, secrets, and agent config.

Shared by CLI, API, and MCP server.  Each caller passes a ``WorkerConfig``
(loaded from ``WM_*`` env vars) and gets back fully resolved objects ready
for dispatch construction.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

import yaml
from dotenv import dotenv_values

from tanren_core.env.environment_schema import (
    DispatchGitConfig,
    DispatchProvisionerConfig,
    EnvironmentProfile,
    EnvironmentProfileType,
    RemoteExecutionConfig,
    SSHDefaults,
    parse_environment_profiles,
)
from tanren_core.env.gates import resolve_gate_cmd as _resolve_gate_cmd_from_profile
from tanren_core.roles import AgentTool, AuthMode, RoleName
from tanren_core.roles_config import load_roles_config
from tanren_core.schemas import Cli, Phase

if TYPE_CHECKING:
    from tanren_core.worker_config import WorkerConfig


def resolve_profile(
    config: WorkerConfig, project: str, environment_profile: str
) -> EnvironmentProfile:
    """Parse tanren.yml and return the named environment profile.

    For remote profiles without inline ``remote_config``, resolves it from
    ``remote.yml`` + ``roles.yml`` automatically.

    Returns:
        Resolved EnvironmentProfile.

    Raises:
        ValueError: If the profile is not found in tanren.yml.
    """
    tanren_yml = Path(config.github_dir) / project / "tanren.yml"
    if tanren_yml.exists():
        loaded = yaml.safe_load(tanren_yml.read_text()) or {}
        data = loaded if isinstance(loaded, dict) else {}
        profiles = parse_environment_profiles(data)
    else:
        profiles = parse_environment_profiles({})
    profile = profiles.get(environment_profile)
    if profile is None:
        available = sorted(profiles.keys())
        raise ValueError(
            f"Environment profile '{environment_profile}' not found in tanren.yml. "
            f"Available: {available}"
        )

    # For remote profiles, populate remote_config from remote.yml + roles.yml
    if profile.type == EnvironmentProfileType.REMOTE and profile.remote_config is None:
        remote_cfg = resolve_remote_config(config, project)
        profile = profile.model_copy(update={"remote_config": remote_cfg})

    return profile


def resolve_remote_config(config: WorkerConfig, project: str) -> RemoteExecutionConfig:
    """Read remote.yml + roles.yml and build dispatch-carried RemoteExecutionConfig.

    Returns:
        Fully resolved RemoteExecutionConfig for the dispatch payload.

    Raises:
        ValueError: If ``WM_REMOTE_CONFIG`` is not set.
    """
    from tanren_core.remote_config import load_remote_config  # noqa: PLC0415

    if not config.remote_config_path:
        raise ValueError("WM_REMOTE_CONFIG is required for remote profiles")

    remote = load_remote_config(config.remote_config_path)

    # Read bootstrap extra_script content (if configured)
    extra_script = None
    if remote.bootstrap.extra_script:
        script_path = Path(remote.bootstrap.extra_script).expanduser()
        if not script_path.is_absolute():
            config_dir = Path(config.remote_config_path).resolve().parent
            script_path = config_dir / script_path
        if script_path.exists():
            extra_script = script_path.read_text()

    # Resolve required CLIs from roles.yml
    required_clis: tuple[str, ...] = ()
    if config.roles_config_path:
        roles = load_roles_config(config.roles_config_path)
        required_clis = tuple(str(c) for c in roles.required_clis())

    # Look up repo URL for this project
    repo_url = remote.repo_url_for(project) or ""

    return RemoteExecutionConfig(
        ssh=SSHDefaults(
            user=remote.ssh.user,
            key_path=remote.ssh.key_path,
            port=remote.ssh.port,
            connect_timeout=remote.ssh.connect_timeout,
            host_key_policy=remote.ssh.host_key_policy,
            ssh_ready_timeout_secs=remote.ssh.ssh_ready_timeout_secs,
        ),
        git=DispatchGitConfig(
            auth_method=str(remote.git.auth),
            token_env=remote.git.token_env,
        ),
        provisioner=DispatchProvisionerConfig(
            type=str(remote.provisioner.type),
            settings=dict(remote.provisioner.settings),
        ),
        repo_url=repo_url,
        required_clis=required_clis,
        bootstrap_extra_script=extra_script,
    )


def resolve_project_env(config: WorkerConfig, project: str) -> dict[str, str]:
    """Read project ``.env`` file and return key-value pairs.

    Returns:
        Dict of env var key-value pairs from the project .env file.
    """
    env_file = Path(config.github_dir) / project / ".env"
    if not env_file.exists():
        return {}
    values = dotenv_values(env_file)
    return {k: v for k, v in values.items() if v is not None}


async def resolve_cloud_secrets(config: WorkerConfig, project: str) -> dict[str, str]:
    """Fetch cloud secrets for vars with ``source: "secret:X"`` in tanren.yml.

    Returns:
        Dict of secret name to value for vars with cloud secret sources.
    """
    tanren_yml = Path(config.github_dir) / project / "tanren.yml"
    if not tanren_yml.exists():
        return {}

    from tanren_core.env.schema import TanrenConfig  # noqa: PLC0415

    data = yaml.safe_load(tanren_yml.read_text()) or {}
    if not isinstance(data, dict):
        return {}
    try:
        tc = TanrenConfig.model_validate(data)
    except Exception:
        return {}

    if tc.env is None:
        return {}

    has_sources = any(v.source for v in (*tc.env.required, *tc.env.optional))
    if not has_sources:
        return {}

    from tanren_core.env.secret_provider_factory import create_secret_provider  # noqa: PLC0415

    provider = create_secret_provider(tc.secrets)
    result: dict[str, str] = {}
    for var in (*tc.env.required, *tc.env.optional):
        if var.source and var.source.startswith("secret:"):
            secret_name = var.source[len("secret:") :]
            value = await provider.get_secret(secret_name)
            if value is not None:
                result[var.key] = value
    return result


def resolve_required_secrets(profile: EnvironmentProfile) -> tuple[str, ...]:
    """Determine which secret names the dispatch needs based on required CLIs and MCP config.

    Returns:
        Tuple of secret names the daemon must resolve from its environment.
    """
    if profile.remote_config is None:
        return ()

    names: list[str] = []
    for cli in profile.remote_config.required_clis:
        if cli == "claude":
            names.extend(["CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CREDENTIALS_JSON"])
        elif cli == "opencode":
            names.append("OPENCODE_ZAI_API_KEY")
        elif cli == "codex":
            names.append("CODEX_AUTH_JSON")

    # Add MCP secret references (env var refs like $MCP_CONTEXT7_KEY)
    for mcp in profile.mcp.values():
        for val in mcp.headers.values():
            if not val.startswith("$"):
                continue
            names.append(val.lstrip("$").strip("{}"))

    return tuple(names)


def role_for_phase(phase: Phase) -> RoleName:
    """Map a dispatch phase to its role name for agent tool resolution.

    Returns:
        RoleName for the given phase.
    """
    if phase in (Phase.AUDIT_TASK, Phase.AUDIT_SPEC):
        return RoleName.AUDIT
    if phase == Phase.RUN_DEMO:
        return RoleName.FEEDBACK
    if phase == Phase.DO_TASK:
        return RoleName.IMPLEMENTATION
    if phase == Phase.INVESTIGATE:
        return RoleName.CONVERSATION
    return RoleName.DEFAULT


def resolve_agent_tool(config: WorkerConfig, phase: Phase) -> AgentTool:
    """Resolve the CLI tool + auth mode for a given phase via roles.yml.

    Returns:
        AgentTool with cli, auth, and model fields.

    Raises:
        ValueError: If ``WM_ROLES_CONFIG_PATH`` is not set for non-gate phases.
    """
    if phase == Phase.GATE:
        return AgentTool(cli=Cli.BASH, auth=AuthMode.API_KEY)
    if not config.roles_config_path:
        raise ValueError("WM_ROLES_CONFIG_PATH is required for non-gate phases")
    return load_roles_config(config.roles_config_path).resolve(role_for_phase(phase))


def resolve_gate_cmd(
    config: WorkerConfig,
    project: str,
    environment_profile: str,
    phase: Phase,
    gate_cmd: str | None,
) -> str | None:
    """Resolve the gate command for a phase.

    For non-gate phases, returns ``gate_cmd`` unchanged.
    For gate phases, resolves from the profile if not provided.

    Returns:
        The resolved gate command string, or the original gate_cmd for non-gate phases.

    Raises:
        ValueError: If the gate phase has no gate command configured.
    """
    if phase != Phase.GATE:
        return gate_cmd

    resolved = gate_cmd
    if resolved is None:
        profile = resolve_profile(config, project, environment_profile)
        resolved = _resolve_gate_cmd_from_profile(profile, phase)

    normalized = resolved.strip() if resolved is not None else ""
    if not normalized:
        raise ValueError(
            "Gate phase requires a non-empty gate command. "
            "Provide gate_cmd or configure environment.<profile>.gate_cmd in tanren.yml."
        )
    return normalized
