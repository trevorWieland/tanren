"""Process spawning for opencode, codex, and bash (gates).

CLI invocation patterns (VERIFIED live 2026-03-07):

opencode run (do-task, run-demo):
    opencode run --model {model} --dir {worktree_path} "short instruction" -f {prompt_file}
    Verified flags from `opencode run --help`:
      -m, --model   model to use in the format of provider/model
      --dir         directory to run in
      -f, --file    file(s) to ATTACH to message (carries the full prompt)
      message: positional arg — short instruction to read the attached file
      stdin: NOT read by opencode (verified: causes 1s silent exit)
      stdout: captured for signal extraction fallback

codex exec (audit-task, audit-spec):
    codex exec --dangerously-bypass-approvals-and-sandbox --model {model}
        -C {worktree_path} -o {last_msg_file} < prompt_on_stdin
    Verified flags from `codex exec --help`:
      -m, --model   Model the agent should use
      -C, --cd      Tell the agent to use the specified directory as its working root
      --dangerously-bypass-approvals-and-sandbox  Skip all confirmation prompts
      -o, --output-last-message  File where the last message is written
      stdin: prompt read when not provided as argument
      stdout: discarded (noisy)

bash (gates):
    bash -c "{gate_cmd}" (cwd=worktree_path)
    stdout+stderr captured (last 100/300 lines -> gate_output)

All processes: start_new_session=True (Python equiv of setsid).
Timeout: SIGTERM -> 5s grace -> SIGKILL via os.killpg.
"""

import asyncio
import contextlib
import logging
import os
import tempfile
import time
from pathlib import Path
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.schemas import Cli, Dispatch

if TYPE_CHECKING:
    from tanren_core.worker_config import WorkerConfig

logger = logging.getLogger(__name__)


class ProcessResult(BaseModel):
    """Result of a spawned process."""

    model_config = ConfigDict(extra="forbid")

    exit_code: int = Field(..., description="Process exit code")
    stdout: str = Field(default="", description="Captured standard output")
    timed_out: bool = Field(..., description="Whether the process was killed due to timeout")
    duration_secs: int = Field(..., ge=0, description="Wall-clock execution time in seconds")


def assemble_prompt(
    command_file: Path,
    spec_folder: str,
    command_name: str,
    context: str | None,
) -> str:
    """Assembles prompt from command file template with spec folder and context.

    Returns:
        Full prompt string with spec folder and signal instructions appended.
    """
    prompt = command_file.read_text()

    extra_context = context or ""

    prompt = f"""{prompt}

---

Spec folder: {spec_folder}

{extra_context}

---

IMPORTANT: Before exiting, write ONLY your exit signal line to this file: \
{spec_folder}/.agent-status
For example, write exactly one line like \
`{command_name}-status: complete` to that file using your file-writing tool."""

    return prompt


async def spawn_process(
    dispatch: Dispatch,
    worktree_path: Path,
    config: WorkerConfig,
    task_env: dict[str, str] | None = None,
) -> ProcessResult:
    """Spawn a CLI process based on dispatch.cli type.

    Returns:
        ProcessResult with exit code, stdout, and timeout info.
    """
    match dispatch.cli:
        case Cli.OPENCODE:
            return await _spawn_opencode(dispatch, worktree_path, config, task_env)
        case Cli.CODEX:
            return await _spawn_codex(dispatch, worktree_path, config, task_env)
        case Cli.CLAUDE:
            return await _spawn_claude(dispatch, worktree_path, config, task_env)
        case Cli.BASH:
            return await _spawn_bash(dispatch, worktree_path, task_env)


async def _spawn_opencode(
    dispatch: Dispatch,
    worktree_path: Path,
    config: WorkerConfig,
    task_env: dict[str, str] | None = None,
) -> ProcessResult:
    """Spawn opencode run process.

    Returns:
        ProcessResult from the opencode execution.
    """
    command_name = dispatch.phase.value
    command_file = worktree_path / config.commands_dir / f"{command_name}.md"

    prompt = assemble_prompt(command_file, dispatch.spec_folder, command_name, dispatch.context)

    with tempfile.NamedTemporaryFile(suffix=".md", delete=False, mode="w") as f:
        f.write(prompt)
        prompt_file_path = f.name

    try:
        cmd = [config.opencode_path, "run"]
        if dispatch.model:
            cmd.extend(["--model", dispatch.model])
        cmd.extend(["--dir", str(worktree_path)])
        cmd.append("Read the attached file and follow its instructions exactly.")
        cmd.extend(["-f", prompt_file_path])

        logger.info("opencode cmd: %s", cmd)

        return await _run_with_timeout(
            cmd, cwd=worktree_path, stdin_data=None, timeout_secs=dispatch.timeout, env=task_env
        )
    finally:
        Path(prompt_file_path).unlink(missing_ok=True)  # noqa: ASYNC240 — trivial cleanup


async def _spawn_codex(
    dispatch: Dispatch,
    worktree_path: Path,
    config: WorkerConfig,
    task_env: dict[str, str] | None = None,
) -> ProcessResult:
    """Spawn codex exec process.

    Returns:
        ProcessResult from the codex execution.
    """
    command_name = dispatch.phase.value
    command_file = worktree_path / config.commands_dir / f"{command_name}.md"

    prompt = assemble_prompt(command_file, dispatch.spec_folder, command_name, dispatch.context)

    fd, last_msg_file = tempfile.mkstemp(suffix=".txt")
    os.close(fd)

    cmd = [
        config.codex_path,
        "exec",
        "--dangerously-bypass-approvals-and-sandbox",
    ]
    if dispatch.model:
        cmd.extend(["--model", dispatch.model])
    cmd.extend(["-C", str(worktree_path)])
    cmd.extend(["-o", last_msg_file])

    logger.info("codex cmd: %s", cmd)

    # For codex, discard stdout (noisy), capture last message from -o file
    result = await _run_with_timeout(
        cmd,
        cwd=worktree_path,
        stdin_data=prompt,
        timeout_secs=dispatch.timeout,
        discard_stdout=True,
        env=task_env,
    )

    # Read the last message file for stdout content
    try:
        last_msg_path = Path(last_msg_file)
        if last_msg_path.exists():  # noqa: ASYNC240 — trivial file check after process exit
            result.stdout = last_msg_path.read_text()  # noqa: ASYNC240 — trivial sync fs op after async work
            last_msg_path.unlink(missing_ok=True)  # noqa: ASYNC240 — trivial sync fs op after async work
    except Exception:  # noqa: S110 — intentional silent exception during cleanup
        pass

    return result


async def _spawn_bash(
    dispatch: Dispatch,
    worktree_path: Path,
    task_env: dict[str, str] | None = None,
) -> ProcessResult:
    """Spawn bash gate process.

    Returns:
        ProcessResult from the gate command.
    """
    if not dispatch.gate_cmd:
        return ProcessResult(
            exit_code=1, stdout="No gate_cmd provided", timed_out=False, duration_secs=0
        )

    cmd = ["bash", "-c", dispatch.gate_cmd]

    return await _run_with_timeout(
        cmd, cwd=worktree_path, stdin_data=None, timeout_secs=dispatch.timeout, env=task_env
    )


async def _spawn_claude(
    dispatch: Dispatch,
    worktree_path: Path,
    config: WorkerConfig,
    task_env: dict[str, str] | None = None,
) -> ProcessResult:
    """Spawn Claude Code process. Uses -p (print mode) with prompt on stdin.

    Returns:
        ProcessResult from the Claude Code execution.
    """
    command_name = dispatch.phase.value
    command_file = worktree_path / config.commands_dir / f"{command_name}.md"

    prompt = assemble_prompt(command_file, dispatch.spec_folder, command_name, dispatch.context)

    cmd = [config.claude_path, "-p", "--dangerously-skip-permissions"]
    if dispatch.model:
        cmd.extend(["--model", dispatch.model])

    logger.info("claude cmd: %s", cmd)

    return await _run_with_timeout(
        cmd, cwd=worktree_path, stdin_data=prompt, timeout_secs=dispatch.timeout, env=task_env
    )


async def _run_with_timeout(
    cmd: list[str],
    cwd: Path,
    stdin_data: str | None,
    timeout_secs: int,
    discard_stdout: bool = False,
    env: dict[str, str] | None = None,
) -> ProcessResult:
    """Run a process with timeout handling.

    Uses start_new_session=True (equiv of setsid).
    On timeout: SIGTERM -> 5s grace -> SIGKILL via os.killpg.
    If env is provided, merges os.environ | env (env wins) so PATH/HOME etc. are preserved.

    Returns:
        ProcessResult with exit code, stdout, and timeout info.
    """
    start_time = time.monotonic()

    stdout_target = asyncio.subprocess.DEVNULL if discard_stdout else asyncio.subprocess.PIPE

    # Merge env: os.environ as base, task_env overrides
    proc_env = None
    if env:
        proc_env = {**os.environ, **env}

    proc = await asyncio.create_subprocess_exec(
        *cmd,
        cwd=str(cwd),
        stdin=asyncio.subprocess.PIPE if stdin_data else None,
        stdout=stdout_target,
        stderr=asyncio.subprocess.STDOUT if not discard_stdout else asyncio.subprocess.DEVNULL,
        start_new_session=True,
        env=proc_env,
    )

    timed_out = False
    stdout_bytes = b""

    try:
        stdin_bytes = stdin_data.encode() if stdin_data else None
        stdout_bytes_raw, _ = await asyncio.wait_for(
            proc.communicate(input=stdin_bytes),
            timeout=timeout_secs,
        )
        if stdout_bytes_raw:
            stdout_bytes = stdout_bytes_raw
    except TimeoutError:
        timed_out = True
        # SIGTERM the process group
        with contextlib.suppress(ProcessLookupError):
            os.killpg(proc.pid, 15)  # SIGTERM

        # Grace period
        try:
            await asyncio.wait_for(proc.wait(), timeout=5)
        except TimeoutError:
            # SIGKILL the process group
            with contextlib.suppress(ProcessLookupError):
                os.killpg(proc.pid, 9)  # SIGKILL
            with contextlib.suppress(Exception):
                await proc.wait()

    duration = int(time.monotonic() - start_time)

    exit_code = proc.returncode if proc.returncode is not None else -1

    return ProcessResult(
        exit_code=exit_code,
        stdout=stdout_bytes.decode(errors="replace") if stdout_bytes else "",
        timed_out=timed_out,
        duration_secs=duration,
    )
