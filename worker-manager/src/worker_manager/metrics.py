"""Plan.md metrics and spec.md integrity guard."""

import asyncio
import hashlib
import re
from pathlib import Path


async def count_unchecked_tasks(plan_path: Path) -> int:
    """Count unchecked task checkboxes in plan.md: lines matching '- [ ] Task N'."""

    def _count() -> int:
        if not plan_path.exists():
            return 0
        content = plan_path.read_text()
        return len(re.findall(r"^\s*- \[ \] Task \d+", content, re.MULTILINE))

    return await asyncio.to_thread(_count)


async def compute_plan_hash(plan_path: Path) -> str:
    """Compute first 8 hex chars of MD5 of plan.md content."""

    def _hash() -> str:
        if not plan_path.exists():
            return "00000000"
        content = plan_path.read_bytes()
        return hashlib.md5(content).hexdigest()[:8]

    return await asyncio.to_thread(_hash)


async def snapshot_spec(spec_path: Path) -> tuple[str, str]:
    """Snapshot spec.md: returns (md5_hex, content) for later comparison."""

    def _snapshot() -> tuple[str, str]:
        if not spec_path.exists():
            return ("", "")
        content = spec_path.read_text()
        md5 = hashlib.md5(content.encode()).hexdigest()
        return (md5, content)

    return await asyncio.to_thread(_snapshot)


async def guard_spec_md(
    spec_path: Path,
    original_md5: str,
    backup_content: str,
) -> bool:
    """Check if spec.md was modified; if so, revert and amend the commit.

    Returns True if spec.md was modified and reverted, False otherwise.
    """

    def _guard() -> bool | None:
        if not spec_path.exists() or not original_md5:
            return False
        current_content = spec_path.read_text()
        current_md5 = hashlib.md5(current_content.encode()).hexdigest()
        if current_md5 == original_md5:
            return False
        # Spec was modified — revert it
        spec_path.write_text(backup_content)
        return None  # Needs git operations in async context

    result = await asyncio.to_thread(_guard)
    if result is not None:
        return result

    # Spec was modified — run git commit --amend
    proc = await asyncio.create_subprocess_exec(
        "git", "add", str(spec_path),
        cwd=str(spec_path.parent),
        stdout=asyncio.subprocess.DEVNULL,
        stderr=asyncio.subprocess.DEVNULL,
    )
    await proc.wait()

    proc = await asyncio.create_subprocess_exec(
        "git", "commit", "--amend", "--no-edit",
        cwd=str(spec_path.parent),
        stdout=asyncio.subprocess.DEVNULL,
        stderr=asyncio.subprocess.DEVNULL,
    )
    await proc.wait()

    return True
