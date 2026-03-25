"""Shared logic for remote-style execution environments (SSH, Docker).

Contains signal extraction, CLI auth validation, CLI command building, and
the set of push-eligible phases. Both SSHExecutionEnvironment and
DockerExecutionEnvironment import from here to avoid duplication.
"""

from __future__ import annotations

import re
import shlex
from typing import TYPE_CHECKING

from tanren_core.schemas import Cli, Phase
from tanren_core.signals import parse_signal_token

if TYPE_CHECKING:
    from tanren_core.schemas import Dispatch
    from tanren_core.worker_config import WorkerConfig

PUSH_PHASES = frozenset({Phase.DO_TASK, Phase.AUDIT_TASK, Phase.RUN_DEMO, Phase.AUDIT_SPEC})

# Per-CLI auth secrets: at least one key in each group must be resolved.
# Bash/gate dispatches have no auth requirements.
CLI_AUTH_GROUPS: dict[Cli, tuple[tuple[str, ...], ...]] = {
    Cli.CLAUDE: (("CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CREDENTIALS_JSON"),),
    Cli.OPENCODE: (("OPENCODE_ZAI_API_KEY",),),
    Cli.CODEX: (("CODEX_AUTH_JSON",),),
}


def validate_cli_auth(cli: Cli, resolved: dict[str, str], *, phase: str = "") -> None:
    """Ensure at least one auth secret was resolved for the dispatch CLI.

    Raises:
        RuntimeError: If no auth secret is available for the CLI.
    """
    groups = CLI_AUTH_GROUPS.get(cli)
    if groups is None:
        return  # bash/gate — no auth needed
    for group in groups:
        if not any(name in resolved for name in group):
            names = " or ".join(group)
            context = f" (phase {phase} uses {cli.value})" if phase else ""
            raise RuntimeError(
                f"No auth secret resolved for {cli.value}{context}: "
                f"need {names} in daemon environment"
            )


def extract_signal_token(
    command_name: str,
    signal_content: str,
    stdout: str,
) -> str | None:
    """Extract signal token from file content, falling back to stdout.

    Mirrors the two-step resolution in ``signals.extract_signal``:
    1. Parse the ``.agent-status`` file content
    2. Grep stdout for ``{command}-status: {token}`` (last match)

    Returns:
        Signal token string or ``None``.
    """
    if signal_content.strip():
        token = parse_signal_token(command_name, signal_content)
        if token:
            return token
    if stdout:
        pattern = rf"{re.escape(command_name)}-status:\s*(\w[\w-]*)"
        matches = re.findall(pattern, stdout)
        if matches:
            return matches[-1]
    return None


def build_cli_command(dispatch: Dispatch, config: WorkerConfig) -> str:
    """Build the CLI command string for remote execution.

    Returns:
        Shell command string for the agent CLI.

    Raises:
        ValueError: If the CLI type is unsupported or gate_cmd is empty for bash.
    """
    if dispatch.cli.value == "claude":
        cmd = config.claude_path
        cmd += " -p --dangerously-skip-permissions"
        if dispatch.model:
            cmd += f" --model {shlex.quote(dispatch.model)}"
        cmd += " < .tanren-prompt.md"
        return cmd
    if dispatch.cli.value == "bash":
        gate_cmd = (dispatch.gate_cmd or "").strip()
        if gate_cmd:
            return gate_cmd
        raise ValueError("Gate dispatch requires a non-empty gate_cmd when cli=bash")
    if dispatch.cli.value == "opencode":
        cmd = config.opencode_path
        cmd += " run"
        if dispatch.model:
            cmd += f" --model {shlex.quote(dispatch.model)}"
        cmd += " --dir ."
        cmd += ' "Read the attached file and follow its instructions exactly."'
        cmd += " -f .tanren-prompt.md"
        return cmd
    if dispatch.cli.value == "codex":
        cmd = config.codex_path
        cmd += " exec --dangerously-bypass-approvals-and-sandbox"
        if dispatch.model:
            cmd += f" --model {shlex.quote(dispatch.model)}"
        cmd += " -C ."
        cmd += " < .tanren-prompt.md"
        return cmd
    raise ValueError(f"Unsupported CLI for remote execution: {dispatch.cli.value}")


def wrap_for_agent_user(command: str, agent_user: str | None) -> str:
    """Wrap a shell command to run as the agent user via ``su -``.

    Returns:
        The command wrapped with ``su -``, or unchanged when no agent_user is configured.
    """
    if agent_user:
        return f"su - {shlex.quote(agent_user)} -c {shlex.quote(command)}"
    return command
