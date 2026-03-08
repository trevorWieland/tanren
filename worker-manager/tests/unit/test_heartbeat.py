"""Tests for heartbeat module."""

import asyncio
import time
from pathlib import Path

import pytest

from worker_manager.heartbeat import HeartbeatWriter


class TestHeartbeatWriter:
    @pytest.mark.asyncio
    async def test_start_creates_file(self, tmp_path: Path):
        writer = HeartbeatWriter(tmp_path, interval=30.0)
        await writer.start("test-dispatch")
        assert (tmp_path / "test-dispatch.heartbeat").exists()
        await writer.stop("test-dispatch")

    @pytest.mark.asyncio
    async def test_stop_removes_file(self, tmp_path: Path):
        writer = HeartbeatWriter(tmp_path, interval=30.0)
        await writer.start("test-dispatch")
        await writer.stop("test-dispatch")
        assert not (tmp_path / "test-dispatch.heartbeat").exists()

    @pytest.mark.asyncio
    async def test_heartbeat_contains_timestamp(self, tmp_path: Path):
        writer = HeartbeatWriter(tmp_path, interval=30.0)
        await writer.start("test-dispatch")
        content = (tmp_path / "test-dispatch.heartbeat").read_text()
        ts = float(content)
        assert abs(ts - time.time()) < 5  # Within 5 seconds
        await writer.stop("test-dispatch")

    @pytest.mark.asyncio
    async def test_cleanup_stale(self, tmp_path: Path):
        # Create a stale heartbeat (old timestamp)
        stale_file = tmp_path / "old-dispatch.heartbeat"
        stale_file.write_text(str(time.time() - 120))  # 2 minutes old

        # Create a fresh heartbeat
        fresh_file = tmp_path / "fresh-dispatch.heartbeat"
        fresh_file.write_text(str(time.time()))

        writer = HeartbeatWriter(tmp_path)
        await writer.cleanup_stale()

        assert not stale_file.exists()
        assert fresh_file.exists()

    @pytest.mark.asyncio
    async def test_cleanup_stale_nonexistent_dir(self, tmp_path: Path):
        writer = HeartbeatWriter(tmp_path / "nonexistent")
        await writer.cleanup_stale()  # Should not raise

    @pytest.mark.asyncio
    async def test_stop_idempotent(self, tmp_path: Path):
        writer = HeartbeatWriter(tmp_path, interval=30.0)
        await writer.stop("never-started")  # Should not raise

    @pytest.mark.asyncio
    async def test_update_loop(self, tmp_path: Path):
        writer = HeartbeatWriter(tmp_path, interval=0.1)
        await writer.start("test-dispatch")

        initial_content = (tmp_path / "test-dispatch.heartbeat").read_text()
        await asyncio.sleep(0.25)
        updated_content = (tmp_path / "test-dispatch.heartbeat").read_text()

        # Timestamp should have been updated
        assert float(updated_content) >= float(initial_content)
        await writer.stop("test-dispatch")
