"""Tests for dispatch builder — CLI/auth resolution logic."""

from __future__ import annotations

import pytest

from tanren_core.dispatch_builder import _resolve_cli_auth
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli, Phase
from tanren_core.worker_config import WorkerConfig


def _minimal_config(tmp_path) -> WorkerConfig:
    """Build a minimal WorkerConfig for testing."""
    return WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        db_url=str(tmp_path / "test.db"),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
    )


class TestResolveCLIAuth:
    def test_gate_phase_resolves_to_bash(self, tmp_path) -> None:
        config = _minimal_config(tmp_path)
        cli, auth, model = _resolve_cli_auth(
            config=config, phase=Phase.GATE, cli=None, auth=None, model=None
        )
        assert cli == Cli.BASH
        assert auth == AuthMode.API_KEY
        assert model is None

    def test_explicit_cli_passes_through(self, tmp_path) -> None:
        config = _minimal_config(tmp_path)
        cli, auth, _model = _resolve_cli_auth(
            config=config,
            phase=Phase.DO_TASK,
            cli=Cli.CLAUDE,
            auth=AuthMode.API_KEY,
            model=None,
        )
        assert cli == Cli.CLAUDE
        assert auth == AuthMode.API_KEY

    def test_non_gate_without_roles_config_raises(self, tmp_path) -> None:
        config = _minimal_config(tmp_path)
        with pytest.raises(ValueError, match="WM_ROLES_CONFIG_PATH"):
            _resolve_cli_auth(
                config=config,
                phase=Phase.DO_TASK,
                cli=None,
                auth=None,
                model=None,
            )
