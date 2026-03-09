"""Post-flight integrity checks run after agent process exits."""

import asyncio
import hashlib
import logging
from dataclasses import dataclass, field
from pathlib import Path

logger = logging.getLogger(__name__)

# Phase-aware protection policy:
# - spec.md: ALWAYS protected (no phase should edit it)
# - plan.md: Protected during do-task, run-demo (implementation phases)
#             NOT protected during audit-task, audit-spec (they append fix items)
# - Makefile, pyproject.toml, .gitignore: warn-only (never reverted, may be legitimate)
_ALWAYS_REVERT = {"spec.md": "spec_reverted"}
_IMPL_PHASE_REVERT = {"plan.md": "plan_reverted"}
_WARN_ONLY = {
    "Makefile": "makefile_modified",
    "pyproject.toml": "deps_modified",
    ".gitignore": "gitignore_modified",
}
_IMPLEMENTATION_PHASES = frozenset({"do-task", "run-demo"})


@dataclass
class PostflightResult:
    integrity_repairs: dict = field(
        default_factory=lambda: {
            "branch_switched": False,
            "spec_reverted": False,
            "plan_reverted": False,
            "makefile_modified": False,
            "deps_modified": False,
            "gitignore_modified": False,
            "wip_committed": False,
        }
    )
    pushed: bool = False
    push_error: str | None = None


async def run_postflight(
    worktree_path: Path,
    branch: str,
    phase: str,
    preflight_hashes: dict[str, str],
    preflight_backups: dict[str, str],
) -> PostflightResult:
    result = PostflightResult()

    # Build phase-aware revert policy
    revert_files = dict(_ALWAYS_REVERT)
    if phase in _IMPLEMENTATION_PHASES:
        revert_files.update(_IMPL_PHASE_REVERT)

    # 1. Branch integrity
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
        logger.critical("Agent switched branch from %s to %s", branch, current)
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(worktree_path),
            "checkout",
            branch,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()
        result.integrity_repairs["branch_switched"] = True

    # 2. Protected file integrity (phase-aware)
    reverted_any = False
    for fname, md5_original in preflight_hashes.items():
        fpath = worktree_path / fname
        if not fpath.exists():
            continue
        current_content = fpath.read_text()
        current_md5 = hashlib.md5(current_content.encode()).hexdigest()
        if current_md5 == md5_original:
            continue

        if fname in revert_files and fname in preflight_backups:
            # Revert: write original content from backup
            fpath.write_text(preflight_backups[fname])
            result.integrity_repairs[revert_files[fname]] = True
            reverted_any = True
            logger.warning("Reverted unauthorized change to %s", fname)
        elif fname in _WARN_ONLY:
            result.integrity_repairs[_WARN_ONLY[fname]] = True
            logger.warning("Agent modified %s — may be legitimate", fname)

    # If any protected files were reverted, commit the reversion
    if reverted_any:
        reverted_names = [
            f
            for f in revert_files
            if result.integrity_repairs.get(revert_files[f], False)
        ]
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(worktree_path),
            "add",
            *reverted_names,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()
        msg = f"postflight: revert {', '.join(reverted_names)} modified by agent"
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(worktree_path),
            "commit",
            "-m",
            msg,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()

    # 3. Uncommitted work
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
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(worktree_path),
            "add",
            "-A",
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(worktree_path),
            "commit",
            "-m",
            "WIP: uncommitted agent work",
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()
        result.integrity_repairs["wip_committed"] = True

    # 4. Git push
    proc = await asyncio.create_subprocess_exec(
        "git",
        "-C",
        str(worktree_path),
        "push",
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    _, stderr = await proc.communicate()
    if proc.returncode == 0:
        result.pushed = True
    else:
        result.pushed = False
        result.push_error = stderr.decode().strip()

    return result
