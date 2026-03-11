"""Tests for CLI/service entrypoints."""

from __future__ import annotations


def test_cli_main_invokes_typer_app(monkeypatch):
    from worker_manager import cli

    called = {"tanren": False}

    def _fake_tanren() -> None:
        called["tanren"] = True

    monkeypatch.setattr(cli, "tanren", _fake_tanren)
    cli.main()

    assert called["tanren"] is True


def test_worker_main_builds_manager_and_runs(monkeypatch):
    from worker_manager import __main__ as main_mod

    class _FakeManager:
        def run(self):
            return "manager-run-coro"

    seen: dict[str, object] = {}
    monkeypatch.setattr(main_mod, "WorkerManager", _FakeManager)

    def _fake_asyncio_run(coro):
        seen["coro"] = coro

    monkeypatch.setattr(main_mod.asyncio, "run", _fake_asyncio_run)
    main_mod.main()

    assert seen["coro"] == "manager-run-coro"
