"""Tests for CLI/service entrypoints."""

from __future__ import annotations

from tanren_cli import main as cli_mod
from tanren_daemon import main as main_mod


def test_cli_main_invokes_typer_app(monkeypatch):

    called = {"tanren": False, "load_config_env": False}

    def _fake_tanren() -> None:
        called["tanren"] = True

    def _fake_load_config_env(*_a, **_kw) -> None:
        called["load_config_env"] = True

    monkeypatch.setattr(cli_mod, "tanren", _fake_tanren)
    monkeypatch.setattr(cli_mod, "load_config_env", _fake_load_config_env)
    cli_mod.main()

    assert called["tanren"] is True
    assert called["load_config_env"] is True


def test_worker_main_builds_manager_and_runs(monkeypatch):
    class _FakeManager:
        def run(self):
            return "manager-run-coro"

    seen: dict[str, object] = {}
    called = {"load_config_env": False}

    monkeypatch.setattr(main_mod, "WorkerManager", _FakeManager)

    def _fake_load_config_env(*_a, **_kw) -> None:
        called["load_config_env"] = True

    monkeypatch.setattr(main_mod, "load_config_env", _fake_load_config_env)

    def _fake_asyncio_run(coro):
        seen["coro"] = coro

    monkeypatch.setattr(main_mod.asyncio, "run", _fake_asyncio_run)
    main_mod.main()

    assert seen["coro"] == "manager-run-coro"
    assert called["load_config_env"] is True
