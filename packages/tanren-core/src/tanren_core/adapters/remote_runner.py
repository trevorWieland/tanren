"""Remote agent runner — executes agent CLI commands on remote VMs."""

from __future__ import annotations

import logging
import shlex
import time
from typing import TYPE_CHECKING

from tanren_core.adapters.remote_types import RemoteAgentResult, WorkspacePath

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import RemoteConnection

logger = logging.getLogger(__name__)


class RemoteAgentRunner:
    """Run agent CLI commands on a remote VM.

    Uploads prompt files, executes the agent with secret sourcing,
    and extracts signal content from the remote filesystem.
    """

    async def run(
        self,
        conn: RemoteConnection,
        workspace: WorkspacePath,
        *,
        prompt_content: str,
        cli_command: str,
        signal_path: str,
        timeout: int = 1800,  # noqa: ASYNC109 — passed through to conn.run, not blocking sleep
    ) -> RemoteAgentResult:
        """Execute an agent command on the remote VM.

        Args:
            conn: RemoteConnection to the VM.
            workspace: WorkspacePath with project info.
            prompt_content: Content of the prompt file to upload.
            cli_command: The CLI command to execute.
            signal_path: Remote path to read signal from after execution.
            timeout: Maximum execution time in seconds.

        Returns:
            RemoteAgentResult with exit code, stdout, signal content.
        """
        start = time.monotonic()

        # Upload prompt file
        prompt_path = f"{workspace.path}/.tanren-prompt.md"
        await conn.upload_content(prompt_content, prompt_path)

        # Build command with PATH augmentation and secret sourcing
        ws = shlex.quote(workspace.path)
        command = (
            f'export PATH="$HOME/.opencode/bin:$HOME/.local/bin:$PATH" && '
            f"set -a && "
            f"source /workspace/.developer-secrets 2>/dev/null; "
            f"source {ws}/.env 2>/dev/null; "
            f"set +a && "
            f"cd {ws} && "
            f"{cli_command}"
        )

        logger.info("Executing remote agent: %s", cli_command)
        result = await conn.run(command, timeout=timeout)

        duration = int(time.monotonic() - start)

        # Extract signal (returns None if agent deleted the file)
        signal_content = await conn.download_content(signal_path) or ""

        # Clean up prompt file
        await conn.run(f"rm -f {shlex.quote(prompt_path)}", timeout=10)

        return RemoteAgentResult(
            exit_code=result.exit_code,
            stdout=result.stdout,
            timed_out=result.timed_out,
            duration_secs=duration,
            stderr=result.stderr,
            signal_content=signal_content,
        )
