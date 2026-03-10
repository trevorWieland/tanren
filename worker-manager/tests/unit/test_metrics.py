"""Tests for metrics module."""

from pathlib import Path

import pytest

from worker_manager.metrics import compute_plan_hash, count_unchecked_tasks


class TestCountUncheckedTasks:
    @pytest.mark.asyncio
    async def test_counts_unchecked(self, tmp_path: Path):
        plan = tmp_path / "plan.md"
        plan.write_text(
            "# Plan\n"
            "- [x] Task 1: Done\n"
            "- [ ] Task 2: Pending\n"
            "- [ ] Task 3: Also pending\n"
            "  - [ ] Fix: Subtask\n"
        )
        assert await count_unchecked_tasks(plan) == 2

    @pytest.mark.asyncio
    async def test_no_tasks(self, tmp_path: Path):
        plan = tmp_path / "plan.md"
        plan.write_text("# Plan\nAll done.\n")
        assert await count_unchecked_tasks(plan) == 0

    @pytest.mark.asyncio
    async def test_missing_file(self, tmp_path: Path):
        assert await count_unchecked_tasks(tmp_path / "plan.md") == 0

    @pytest.mark.asyncio
    async def test_indented_tasks(self, tmp_path: Path):
        plan = tmp_path / "plan.md"
        plan.write_text("  - [ ] Task 1: Indented\n")
        assert await count_unchecked_tasks(plan) == 1

    @pytest.mark.asyncio
    async def test_does_not_count_fix_items(self, tmp_path: Path):
        plan = tmp_path / "plan.md"
        plan.write_text("- [ ] Task 1: Something\n  - [ ] Fix: not a Task N\n")
        assert await count_unchecked_tasks(plan) == 1


class TestComputePlanHash:
    @pytest.mark.asyncio
    async def test_returns_8_hex_chars(self, tmp_path: Path):
        plan = tmp_path / "plan.md"
        plan.write_text("content")
        h = await compute_plan_hash(plan)
        assert len(h) == 8
        int(h, 16)  # Should parse as hex

    @pytest.mark.asyncio
    async def test_missing_file(self, tmp_path: Path):
        h = await compute_plan_hash(tmp_path / "plan.md")
        assert h == "00000000"

    @pytest.mark.asyncio
    async def test_deterministic(self, tmp_path: Path):
        plan = tmp_path / "plan.md"
        plan.write_text("same content")
        h1 = await compute_plan_hash(plan)
        h2 = await compute_plan_hash(plan)
        assert h1 == h2

    @pytest.mark.asyncio
    async def test_changes_with_content(self, tmp_path: Path):
        plan = tmp_path / "plan.md"
        plan.write_text("content A")
        h1 = await compute_plan_hash(plan)
        plan.write_text("content B")
        h2 = await compute_plan_hash(plan)
        assert h1 != h2
