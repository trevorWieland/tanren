"""Tests for ConfigService — dynamic worker lanes from WorkerConfig."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from tanren_api.services.core import ConfigService
from tanren_core.worker_config import WorkerConfig

if TYPE_CHECKING:
    from pathlib import Path


def _make_worker_config(tmp_path: Path) -> WorkerConfig:
    return WorkerConfig(
        db_url=str(tmp_path / "test.db"),
        ipc_dir=str(tmp_path),
        github_dir=str(tmp_path),
        data_dir=str(tmp_path),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
        max_impl=2,
        max_audit=3,
        max_gate=5,
        max_provision=8,
    )


@pytest.mark.asyncio
async def test_config_dynamic_worker_lanes(tmp_path: Path) -> None:
    """ConfigService returns dynamic worker_lanes when WorkerConfig is wired."""
    from tanren_api.settings import APISettings
    from tanren_core.store.sqlite import SqliteStore

    store = SqliteStore(tmp_path / "cfg.db")
    await store._ensure_conn()
    try:
        settings = APISettings(api_key="k", db_url=str(tmp_path / "cfg.db"))
        wc = _make_worker_config(tmp_path)
        svc = ConfigService(settings, store, worker_config=wc)
        resp = await svc.get()
        assert resp.worker_lanes == {"impl": 2, "audit": 3, "gate": 5, "provision": 8}
    finally:
        await store.close()


@pytest.mark.asyncio
async def test_config_default_lanes_without_worker_config(tmp_path: Path) -> None:
    """ConfigService falls back to defaults when WorkerConfig is None."""
    from tanren_api.settings import APISettings
    from tanren_core.store.sqlite import SqliteStore

    store = SqliteStore(tmp_path / "cfg2.db")
    await store._ensure_conn()
    try:
        settings = APISettings(api_key="k", db_url=str(tmp_path / "cfg2.db"))
        svc = ConfigService(settings, store)
        resp = await svc.get()
        assert resp.worker_lanes == {"impl": 1, "audit": 1, "gate": 3, "provision": 10}
    finally:
        await store.close()
