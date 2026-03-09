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
    stdout+stderr captured (last 100 lines -> gate_output)

All processes: start_new_session=True (Python equiv of setsid).
Timeout: SIGTERM -> 5s grace -> SIGKILL via os.killpg.
"""

import asyncio
import contextlib
import logging
import os
import tempfile
from dataclasses import dataclass
from pathlib import Path

from worker_manager.config import Config
from worker_manager.schemas import Cli, Dispatch

logger = logging.getLogger(__name__)


@dataclass
class ProcessResult:
    """Result of a spawned process."""

    exit_code: int
    stdout: str
    timed_out: bool
    duration_secs: int


def assemble_prompt(
    command_file: Path,
    spec_folder: str,
    command_name: str,
    context: str | None,
) -> str:
    """Assemble prompt from command file + context, matching orchestrate.sh lines 337-350."""
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
    config: Config,
) -> ProcessResult:
    """Spawn a CLI process based on dispatch.cli type."""
    match dispatch.cli:
        case Cli.OPENCODE:
            return await _spawn_opencode(dispatch, worktree_path, config)
        case Cli.CODEX:
            return await _spawn_codex(dispatch, worktree_path, config)
        case Cli.BASH:
            return await _spawn_bash(dispatch, worktree_path)


async def _spawn_opencode(
    dispatch: Dispatch,
    worktree_path: Path,
    config: Config,
) -> ProcessResult:
    """Spawn opencode run process."""
    command_name = dispatch.phase.value
    command_file = worktree_path / config.commands_dir / f"{command_name}.md"

    prompt = assemble_prompt(
        command_file, dispatch.spec_folder, command_name, dispatch.context
    )

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
            cmd, cwd=worktree_path, stdin_data=None, timeout=dispatch.timeout
        )
    finally:
        Path(prompt_file_path).unlink(missing_ok=True)


async def _spawn_codex(
    dispatch: Dispatch,
    worktree_path: Path,
    config: Config,
) -> ProcessResult:
    """Spawn codex exec process."""
    command_name = dispatch.phase.value
    command_file = worktree_path / config.commands_dir / f"{command_name}.md"

    prompt = assemble_prompt(
        command_file, dispatch.spec_folder, command_name, dispatch.context
    )

    last_msg_file = tempfile.mktemp(suffix=".txt")

    cmd = [
        config.codex_path, "exec",
        "--dangerously-bypass-approvals-and-sandbox",
    ]
    if dispatch.model:
        cmd.extend(["--model", dispatch.model])
    cmd.extend(["-C", str(worktree_path)])
    cmd.extend(["-o", last_msg_file])

    logger.info("codex cmd: %s", cmd)

    # For codex, discard stdout (noisy), capture last message from -o file
    result = await _run_with_timeout(
        cmd, cwd=worktree_path, stdin_data=prompt, timeout=dispatch.timeout,
        discard_stdout=True,
    )

    # Read the last message file for stdout content
    try:
        last_msg_path = Path(last_msg_file)
        if last_msg_path.exists():
            result.stdout = last_msg_path.read_text()
            last_msg_path.unlink(missing_ok=True)
    except Exception:
        pass

    return result


async def _spawn_bash(
    dispatch: Dispatch,
    worktree_path: Path,
) -> ProcessResult:
    """Spawn bash gate process."""
    if not dispatch.gate_cmd:
        return ProcessResult(
            exit_code=1, stdout="No gate_cmd provided", timed_out=False, duration_secs=0
        )

    cmd = ["bash", "-c", dispatch.gate_cmd]

    return await _run_with_timeout(
        cmd, cwd=worktree_path, stdin_data=None, timeout=dispatch.timeout
    )


async def _run_with_timeout(
    cmd: list[str],
    cwd: Path,
    stdin_data: str | None,
    timeout: int,
    discard_stdout: bool = False,
) -> ProcessResult:
    """Run a process with timeout handling.

    Uses start_new_session=True (equiv of setsid).
    On timeout: SIGTERM -> 5s grace -> SIGKILL via os.killpg.
    """
    import time

    start_time = time.monotonic()

    stdout_target = asyncio.subprocess.DEVNULL if discard_stdout else asyncio.subprocess.PIPE

    proc = await asyncio.create_subprocess_exec(
        *cmd,
        cwd=str(cwd),
        stdin=asyncio.subprocess.PIPE if stdin_data else None,
        stdout=stdout_target,
        stderr=asyncio.subprocess.STDOUT if not discard_stdout else asyncio.subprocess.DEVNULL,
        start_new_session=True,
    )

    timed_out = False
    stdout_bytes = b""

    try:
        stdin_bytes = stdin_data.encode() if stdin_data else None
        stdout_bytes_raw, _ = await asyncio.wait_for(
            proc.communicate(input=stdin_bytes),
            timeout=timeout,
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
