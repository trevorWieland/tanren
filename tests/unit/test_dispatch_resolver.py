"""Tests for dispatch_resolver — shared resolution logic."""

from __future__ import annotations

from pathlib import Path

import pytest

from tanren_core.dispatch_resolver import (
    resolve_agent_tool,
    resolve_gate_cmd,
    resolve_profile,
    resolve_project_env,
    resolve_required_secrets,
    role_for_phase,
)
from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    EnvironmentProfileType,
    McpServerConfig,
)
from tanren_core.schemas import Cli, Phase
from tanren_core.worker_config import WorkerConfig

_ROLES_YML = """\
agents:
  default:
    cli: claude
    auth: subscription
    model: opus
  implementation:
    cli: opencode
    auth: api_key
    model: zai-coding-plan/glm-5
  audit:
    cli: codex
    auth: subscription
    model: gpt-5.3-codex
"""


def _make_config(tmp_path: Path, *, roles: bool = True) -> WorkerConfig:
    github_dir = tmp_path / "github"
    github_dir.mkdir()
    roles_path = None
    if roles:
        roles_file = tmp_path / "roles.yml"
        roles_file.write_text(_ROLES_YML)
        roles_path = str(roles_file)
    return WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(github_dir),
        data_dir=str(tmp_path / "data"),
        db_url=str(tmp_path / "events.db"),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
        roles_config_path=roles_path,
    )


class TestResolveProfile:
    def test_reads_local_profile(self, tmp_path: Path):
        config = _make_config(tmp_path)
        project_dir = Path(config.github_dir) / "myproj"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text(
            "environment:\n  dev:\n    type: local\n    gate_cmd: make check\n"
        )

        profile = resolve_profile(config, "myproj", "dev")

        assert profile.name == "dev"
        assert profile.type == EnvironmentProfileType.LOCAL
        assert profile.gate_cmd == "make check"

    def test_raises_on_missing_profile(self, tmp_path: Path):
        config = _make_config(tmp_path)
        project_dir = Path(config.github_dir) / "myproj"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text("environment:\n  dev:\n    type: local\n")

        with pytest.raises(ValueError, match=r"not found in tanren\.yml"):
            resolve_profile(config, "myproj", "nonexistent")

    def test_raises_when_no_tanren_yml(self, tmp_path: Path):
        config = _make_config(tmp_path)
        (Path(config.github_dir) / "myproj").mkdir()

        with pytest.raises(ValueError, match="not found"):
            resolve_profile(config, "myproj", "default")


class TestResolveProjectEnv:
    def test_reads_dotenv(self, tmp_path: Path):
        config = _make_config(tmp_path)
        project_dir = Path(config.github_dir) / "myproj"
        project_dir.mkdir()
        (project_dir / ".env").write_text("KEY_A=val_a\nKEY_B=val_b\n")

        result = resolve_project_env(config, "myproj")

        assert result == {"KEY_A": "val_a", "KEY_B": "val_b"}

    def test_returns_empty_when_no_env(self, tmp_path: Path):
        config = _make_config(tmp_path)
        (Path(config.github_dir) / "myproj").mkdir()

        result = resolve_project_env(config, "myproj")

        assert result == {}


class TestResolveRequiredSecrets:
    def test_local_profile_returns_empty(self):
        profile = EnvironmentProfile(name="dev", type=EnvironmentProfileType.LOCAL)
        assert resolve_required_secrets(profile) == ()

    def test_claude_secrets(self):
        from tanren_core.env.environment_schema import (
            DispatchProvisionerConfig,
            RemoteExecutionConfig,
        )

        profile = EnvironmentProfile(
            name="prod",
            type=EnvironmentProfileType.REMOTE,
            remote_config=RemoteExecutionConfig(
                provisioner=DispatchProvisionerConfig(type="manual", settings={}),
                repo_url="https://github.com/test.git",
                required_clis=("claude",),
            ),
        )
        secrets = resolve_required_secrets(profile)
        assert "CLAUDE_CODE_OAUTH_TOKEN" in secrets
        assert "CLAUDE_CREDENTIALS_JSON" in secrets

    def test_mcp_headers(self):
        from tanren_core.env.environment_schema import (
            DispatchProvisionerConfig,
            RemoteExecutionConfig,
        )

        profile = EnvironmentProfile(
            name="prod",
            type=EnvironmentProfileType.REMOTE,
            remote_config=RemoteExecutionConfig(
                provisioner=DispatchProvisionerConfig(type="manual", settings={}),
                repo_url="https://github.com/test.git",
                required_clis=(),
            ),
            mcp={
                "ctx7": McpServerConfig(
                    url="https://mcp.example.com", headers={"x-api-key": "$MCP_CTX7_KEY"}
                )
            },
        )
        secrets = resolve_required_secrets(profile)
        assert "MCP_CTX7_KEY" in secrets


class TestResolveAgentTool:
    def test_gate_returns_bash(self, tmp_path: Path):
        config = _make_config(tmp_path)
        tool = resolve_agent_tool(config, Phase.GATE)
        assert tool.cli == Cli.BASH

    def test_do_task_reads_roles(self, tmp_path: Path):
        config = _make_config(tmp_path)
        tool = resolve_agent_tool(config, Phase.DO_TASK)
        assert tool.cli == Cli.OPENCODE

    def test_audit_reads_roles(self, tmp_path: Path):
        config = _make_config(tmp_path)
        tool = resolve_agent_tool(config, Phase.AUDIT_TASK)
        assert tool.cli == Cli.CODEX

    def test_raises_without_roles_config(self, tmp_path: Path):
        config = _make_config(tmp_path, roles=False)
        with pytest.raises(ValueError, match="WM_ROLES_CONFIG_PATH"):
            resolve_agent_tool(config, Phase.DO_TASK)


class TestRoleForPhase:
    def test_mappings(self):
        from tanren_core.roles import RoleName

        assert role_for_phase(Phase.DO_TASK) == RoleName.IMPLEMENTATION
        assert role_for_phase(Phase.AUDIT_TASK) == RoleName.AUDIT
        assert role_for_phase(Phase.AUDIT_SPEC) == RoleName.AUDIT
        assert role_for_phase(Phase.RUN_DEMO) == RoleName.FEEDBACK
        assert role_for_phase(Phase.INVESTIGATE) == RoleName.CONVERSATION
        assert role_for_phase(Phase.GATE) == RoleName.DEFAULT


class TestResolveGateCmd:
    def test_non_gate_returns_input(self, tmp_path: Path):
        config = _make_config(tmp_path)
        result = resolve_gate_cmd(config, "myproj", "dev", Phase.DO_TASK, "ignored")
        assert result == "ignored"

    def test_gate_uses_provided_cmd(self, tmp_path: Path):
        config = _make_config(tmp_path)
        result = resolve_gate_cmd(config, "myproj", "dev", Phase.GATE, "make test")
        assert result == "make test"

    def test_gate_raises_when_empty(self, tmp_path: Path):
        config = _make_config(tmp_path)
        project_dir = Path(config.github_dir) / "myproj"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text(
            "environment:\n  dev:\n    type: local\n    gate_cmd: ''\n"
        )

        with pytest.raises(ValueError, match="Gate phase requires"):
            resolve_gate_cmd(config, "myproj", "dev", Phase.GATE, None)
