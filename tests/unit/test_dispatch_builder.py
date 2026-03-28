"""Tests for dispatch builder — unified resolution logic."""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from tanren_core.dispatch_builder import (
    ResolvedInputs,
    resolve_dispatch_inputs,
    resolve_provision_inputs,
)
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli, Phase
from tanren_core.worker_config import WorkerConfig


def _config(tmp_path) -> WorkerConfig:
    return WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        db_url=str(tmp_path / "test.db"),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
    )


def _mock_resolver(tanren_config: dict | None = None) -> AsyncMock:
    resolver = AsyncMock()
    resolver.load_tanren_config.return_value = tanren_config or {
        "environment": {"default": {"type": "local", "gate_cmd": "make check"}}
    }
    resolver.load_project_env.return_value = {"FOO": "bar"}
    return resolver


class TestResolveDispatchInputs:
    async def test_resolves_profile_from_resolver(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        result = await resolve_dispatch_inputs(
            resolver=resolver,
            config=config,
            project="test",
            phase=Phase.GATE,
            branch="main",
        )
        assert isinstance(result, ResolvedInputs)
        assert result.profile.name == "default"
        assert result.profile.type.value == "local"

    async def test_uses_pre_resolved_profile(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        profile = EnvironmentProfile(name="custom")
        result = await resolve_dispatch_inputs(
            resolver=resolver,
            config=config,
            project="test",
            phase=Phase.GATE,
            branch="main",
            resolved_profile=profile,
        )
        assert result.profile.name == "custom"
        # Should NOT call load_tanren_config for profile resolution
        # (but may call it for cloud secrets if not overridden)

    async def test_uses_pre_resolved_env(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        result = await resolve_dispatch_inputs(
            resolver=resolver,
            config=config,
            project="test",
            phase=Phase.GATE,
            branch="main",
            project_env={"CUSTOM": "value"},
        )
        assert result.project_env == {"CUSTOM": "value"}

    async def test_gate_phase_resolves_bash(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        result = await resolve_dispatch_inputs(
            resolver=resolver,
            config=config,
            project="test",
            phase=Phase.GATE,
            branch="main",
        )
        assert result.cli == Cli.BASH
        assert result.auth == AuthMode.API_KEY

    async def test_explicit_cli_passes_through(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        result = await resolve_dispatch_inputs(
            resolver=resolver,
            config=config,
            project="test",
            phase=Phase.GATE,
            branch="main",
            cli=Cli.CLAUDE,
            auth=AuthMode.API_KEY,
        )
        assert result.cli == Cli.CLAUDE

    async def test_missing_profile_raises(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver({"environment": {}})
        with pytest.raises(ValueError, match="not found"):
            await resolve_dispatch_inputs(
                resolver=resolver,
                config=config,
                project="test",
                phase=Phase.GATE,
                branch="main",
                environment_profile="nonexistent",
            )

    async def test_gate_cmd_resolved_from_profile(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        result = await resolve_dispatch_inputs(
            resolver=resolver,
            config=config,
            project="test",
            phase=Phase.GATE,
            branch="main",
        )
        assert result.gate_cmd == "make check"

    async def test_explicit_gate_cmd_passes_through(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        result = await resolve_dispatch_inputs(
            resolver=resolver,
            config=config,
            project="test",
            phase=Phase.GATE,
            branch="main",
            gate_cmd="custom-check",
        )
        assert result.gate_cmd == "custom-check"


class TestResolveProvisionInputs:
    async def test_resolves_profile(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        result = await resolve_provision_inputs(
            resolver=resolver,
            config=config,
            project="test",
            branch="main",
        )
        assert isinstance(result, ResolvedInputs)
        assert result.profile.name == "default"
        assert result.cli == Cli.CLAUDE
        assert result.auth == AuthMode.API_KEY

    async def test_uses_pre_resolved_profile(self, tmp_path) -> None:
        config = _config(tmp_path)
        resolver = _mock_resolver()
        profile = EnvironmentProfile(name="custom")
        result = await resolve_provision_inputs(
            resolver=resolver,
            config=config,
            project="test",
            resolved_profile=profile,
        )
        assert result.profile.name == "custom"
