"""Plan.md metrics."""

import asyncio
import hashlib
import re
from pathlib import Path


async def count_unchecked_tasks(plan_path: Path) -> int:
    """Count unchecked task checkboxes in plan.md: lines matching '- [ ] Task N'.

    Returns:
        Number of unchecked task lines.
    """

    def _count() -> int:
        if not plan_path.exists():
            return 0
        content = plan_path.read_text()
        return len(re.findall(r"^\s*- \[ \] Task \d+", content, re.MULTILINE))

    return await asyncio.to_thread(_count)


async def compute_plan_hash(plan_path: Path) -> str:
    """Compute first 8 hex chars of MD5 of plan.md content.

    Returns:
        8-char hex hash, or ``"00000000"`` if file does not exist.
    """

    def _hash() -> str:
        if not plan_path.exists():
            return "00000000"
        content = plan_path.read_bytes()
        return hashlib.md5(content).hexdigest()[:8]

    return await asyncio.to_thread(_hash)
