"""Tests for CLI/service entrypoints."""

from __future__ import annotations


def test_cli_main_invokes_typer_app(monkeypatch):
    from worker_manager import cli

    called = {"tanren": False, "load_config_env": False}

    def _fake_tanren() -> None:
        called["tanren"] = True

    def _fake_load_config_env(*_a, **_kw) -> None:
        called["load_config_env"] = True

    monkeypatch.setattr(cli, "tanren", _fake_tanren)
    monkeypatch.setattr(cli, "load_config_env", _fake_load_config_env)
    cli.main()

    assert called["tanren"] is True
    assert called["load_config_env"] is True


def test_worker_main_builds_manager_and_runs(monkeypatch):
    from worker_manager import __main__ as main_mod

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
