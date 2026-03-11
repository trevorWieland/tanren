"""Pre-flight checks run before spawning an agent process."""

import asyncio
import hashlib
import logging
from datetime import UTC, datetime
from pathlib import Path

from pydantic import BaseModel, ConfigDict, Field

logger = logging.getLogger(__name__)

# All files snapshotted for comparison; which ones get reverted is phase-dependent (see postflight)
SNAPSHOT_FILES = ("spec.md", "plan.md", "Makefile", "pyproject.toml", ".gitignore")


class PreflightResult(BaseModel):
    """Outcome of pre-flight checks before process execution."""

    model_config = ConfigDict(extra="forbid")

    passed: bool = Field(...)
    repairs: list[str] = Field(default_factory=list)
    error: str | None = Field(default=None)
    file_hashes: dict[str, str] = Field(default_factory=dict)
    file_backups: dict[str, str] = Field(default_factory=dict)


async def run_preflight(
    worktree_path: Path,
    branch: str,
    spec_folder: Path,
    phase: str,
) -> PreflightResult:
    result = PreflightResult(passed=True)

    # 1. Branch verification
    proc = await asyncio.create_subprocess_exec(
        "git",
        "-C",
        str(worktree_path),
        "branch",
        "--show-current",
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, _ = await proc.communicate()
    current = stdout.decode().strip()
    if current != branch:
        logger.warning("Wrong branch %s, expected %s — checking out", current, branch)
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(worktree_path),
            "checkout",
            branch,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        _, stderr = await proc.communicate()
        if proc.returncode != 0:
            result.passed = False
            result.error = f"Cannot checkout {branch}: {stderr.decode().strip()}"
            return result
        result.repairs.append(f"Switched branch from {current} to {branch}")

    # 2. Clean working tree
    proc = await asyncio.create_subprocess_exec(
        "git",
        "-C",
        str(worktree_path),
        "status",
        "--porcelain",
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, _ = await proc.communicate()
    if stdout.decode().strip():
        ts = datetime.now(UTC).strftime("%Y%m%d-%H%M%S")
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(worktree_path),
            "stash",
            "push",
            "--include-untracked",
            "-m",
            f"preflight-{ts}",
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()
        result.repairs.append(f"Stashed dirty working tree (preflight-{ts})")

    # 3. Protected file snapshot (all files, always — phase determines revert policy in postflight)
    for fname in SNAPSHOT_FILES:
        fpath = worktree_path / fname
        if fpath.exists():
            content = fpath.read_text()
            md5 = hashlib.md5(content.encode()).hexdigest()
            result.file_hashes[fname] = md5
            result.file_backups[fname] = content

    # 4. Clear agent status files
    for name in (".agent-status", "audit-findings.json", "investigation-report.json"):
        fpath = spec_folder / name
        if fpath.exists():
            fpath.unlink()
            result.repairs.append(f"Cleared {name}")

    # 5. Verify command file (agent phases only — gates/setup/cleanup don't use command files)
    _AGENT_PHASES = frozenset({"do-task", "audit-task", "run-demo", "audit-spec", "investigate"})
    if phase in _AGENT_PHASES:
        cmd_file = worktree_path / ".claude" / "commands" / "tanren" / f"{phase}.md"
        if not cmd_file.exists():
            result.passed = False
            result.error = f"Command file missing: .claude/commands/tanren/{phase}.md"
            return result

    return result
