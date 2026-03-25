"""Tests for dispatch_lifecycle — _resolve_cli_auth and gate_cmd resolution."""

from __future__ import annotations

import pytest

from tanren_api.models import DispatchRequest
from tanren_api.services.dispatch_lifecycle import _resolve_cli_auth
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli, Phase


def _req(**overrides) -> DispatchRequest:
    defaults = {
        "project": "test",
        "phase": Phase.GATE,
        "branch": "main",
        "spec_folder": ".",
        "resolved_profile": EnvironmentProfile(name="default"),
    }
    return DispatchRequest.model_validate(defaults | overrides)


class TestResolveCLIAuth:
    def test_gate_phase_resolves_to_bash_without_config(self) -> None:
        cli, auth, model = _resolve_cli_auth(_req(phase=Phase.GATE))
        assert cli == Cli.BASH
        assert auth == AuthMode.API_KEY
        assert model is None

    def test_explicit_cli_passes_through(self) -> None:
        cli, auth, _model = _resolve_cli_auth(
            _req(cli=Cli.CLAUDE, auth=AuthMode.API_KEY, phase=Phase.DO_TASK)
        )
        assert cli == Cli.CLAUDE
        assert auth == AuthMode.API_KEY

    def test_non_gate_without_config_raises(self) -> None:
        with pytest.raises(RuntimeError, match="WorkerConfig required"):
            _resolve_cli_auth(_req(phase=Phase.DO_TASK))
