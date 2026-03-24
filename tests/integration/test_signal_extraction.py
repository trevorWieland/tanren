"""Integration tests for remote signal extraction, auth validation, and dispatch resolution."""

from pathlib import Path

import pytest

from tanren_core.adapters.ssh_environment import _extract_signal_token, _validate_cli_auth
from tanren_core.dispatch_resolver import (
    resolve_agent_tool,
    resolve_cloud_secrets,
    resolve_gate_cmd,
    resolve_profile,
    resolve_project_env,
    resolve_remote_config,
    resolve_required_secrets,
    role_for_phase,
)
from tanren_core.schemas import Cli, Phase
from tanren_core.worker_config import WorkerConfig


class TestExtractSignalTokenIntegration:
    """Verify _extract_signal_token covers all dispatch phase signal patterns."""

    def test_do_task_signals(self):
        for signal in ("complete", "blocked", "all-done", "error"):
            token = _extract_signal_token("do-task", f"do-task-status: {signal}", "")
            assert token == signal

    def test_audit_task_signals(self):
        for signal in ("pass", "fail", "error"):
            token = _extract_signal_token("audit-task", f"audit-task-status: {signal}", "")
            assert token == signal

    def test_run_demo_signals(self):
        for signal in ("pass", "fail", "error"):
            token = _extract_signal_token("run-demo", f"run-demo-status: {signal}", "")
            assert token == signal

    def test_stdout_fallback_all_phases(self):
        for cmd in ("do-task", "audit-task", "run-demo"):
            token = _extract_signal_token(cmd, "", f"output\n{cmd}-status: complete\n")
            assert token == "complete"

    def test_file_precedence_over_stdout(self):
        token = _extract_signal_token(
            "do-task", "do-task-status: blocked", "do-task-status: complete"
        )
        assert token == "blocked"


class TestValidateCliAuthIntegration:
    """Verify CLI auth validation for all supported CLIs."""

    def test_claude_requires_at_least_one_auth(self):
        _validate_cli_auth(Cli.CLAUDE, {"CLAUDE_CODE_OAUTH_TOKEN": "tok"})
        _validate_cli_auth(Cli.CLAUDE, {"CLAUDE_CREDENTIALS_JSON": "{}"})
        with pytest.raises(RuntimeError):
            _validate_cli_auth(Cli.CLAUDE, {})

    def test_opencode_requires_api_key(self):
        _validate_cli_auth(Cli.OPENCODE, {"OPENCODE_ZAI_API_KEY": "key"})
        with pytest.raises(RuntimeError):
            _validate_cli_auth(Cli.OPENCODE, {})

    def test_codex_requires_auth_json(self):
        _validate_cli_auth(Cli.CODEX, {"CODEX_AUTH_JSON": "{}"})
        with pytest.raises(RuntimeError):
            _validate_cli_auth(Cli.CODEX, {})

    def test_bash_needs_no_auth(self):
        _validate_cli_auth(Cli.BASH, {})


def _make_config(tmp_path: Path) -> WorkerConfig:
    github_dir = tmp_path / "github"
    github_dir.mkdir(exist_ok=True)
    return WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(github_dir),
        data_dir=str(tmp_path / "data"),
        db_url=str(tmp_path / "events.db"),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
    )


class TestDispatchResolverIntegration:
    """Integration tests for dispatch resolution functions."""

    def test_resolve_profile_local(self, tmp_path: Path):
        config = _make_config(tmp_path)
        project_dir = Path(config.github_dir) / "proj"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text(
            "environment:\n  dev:\n    type: local\n    gate_cmd: make test\n"
        )
        profile = resolve_profile(config, "proj", "dev")
        assert profile.name == "dev"
        assert profile.gate_cmd == "make test"

    def test_resolve_project_env(self, tmp_path: Path):
        config = _make_config(tmp_path)
        project_dir = Path(config.github_dir) / "proj"
        project_dir.mkdir()
        (project_dir / ".env").write_text("A=1\nB=2\n")
        env = resolve_project_env(config, "proj")
        assert env == {"A": "1", "B": "2"}

    def test_resolve_project_env_missing(self, tmp_path: Path):
        config = _make_config(tmp_path)
        (Path(config.github_dir) / "proj").mkdir()
        assert resolve_project_env(config, "proj") == {}

    def test_resolve_required_secrets_local(self):
        from tanren_core.env.environment_schema import EnvironmentProfile

        profile = EnvironmentProfile(name="dev")
        assert resolve_required_secrets(profile) == ()

    def test_resolve_required_secrets_remote_claude(self):
        from tanren_core.env.environment_schema import (
            DispatchProvisionerConfig,
            EnvironmentProfile,
            EnvironmentProfileType,
            RemoteExecutionConfig,
        )

        profile = EnvironmentProfile(
            name="prod",
            type=EnvironmentProfileType.REMOTE,
            remote_config=RemoteExecutionConfig(
                provisioner=DispatchProvisionerConfig(type="manual", settings={}),
                repo_url="https://github.com/test.git",
                required_clis=("claude", "opencode"),
            ),
        )
        secrets = resolve_required_secrets(profile)
        assert "CLAUDE_CODE_OAUTH_TOKEN" in secrets
        assert "CLAUDE_CREDENTIALS_JSON" in secrets
        assert "OPENCODE_ZAI_API_KEY" in secrets

    def test_resolve_profile_raises_missing(self, tmp_path: Path):
        config = _make_config(tmp_path)
        (Path(config.github_dir) / "proj").mkdir()
        (Path(config.github_dir) / "proj" / "tanren.yml").write_text(
            "environment:\n  dev:\n    type: local\n"
        )
        with pytest.raises(ValueError, match="not found"):
            resolve_profile(config, "proj", "nonexistent")

    def test_role_for_phase_mappings(self):
        from tanren_core.roles import RoleName

        assert role_for_phase(Phase.DO_TASK) == RoleName.IMPLEMENTATION
        assert role_for_phase(Phase.AUDIT_TASK) == RoleName.AUDIT
        assert role_for_phase(Phase.GATE) == RoleName.DEFAULT

    def test_resolve_gate_cmd_passthrough(self, tmp_path: Path):
        config = _make_config(tmp_path)
        result = resolve_gate_cmd(config, "proj", "dev", Phase.DO_TASK, "make check")
        assert result == "make check"

    def test_resolve_gate_cmd_raises_when_empty(self, tmp_path: Path):
        config = _make_config(tmp_path)
        project_dir = Path(config.github_dir) / "proj"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text(
            "environment:\n  dev:\n    type: local\n    gate_cmd: ''\n"
        )
        with pytest.raises(ValueError, match="Gate phase requires"):
            resolve_gate_cmd(config, "proj", "dev", Phase.GATE, None)

    def test_resolve_remote_config(self, tmp_path: Path):
        config = _make_config(tmp_path)
        remote_yml = tmp_path / "remote.yml"
        remote_yml.write_text(
            "ssh:\n  user: root\n  key_path: ~/.ssh/test\ngit:\n  auth: token\n"
            "  token_env: GIT_TOKEN\nprovisioner:\n  type: manual\n"
            "  settings:\n    vms:\n      - id: vm-1\n        host: 10.0.0.1\n"
            "repos:\n  - project: proj\n    repo_url: https://github.com/test.git\n"
        )
        roles_yml = tmp_path / "roles.yml"
        roles_yml.write_text(
            "agents:\n  default:\n    cli: claude\n    auth: oauth\n    model: sonnet\n"
        )
        config = WorkerConfig(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            db_url=str(tmp_path / "events.db"),
            worktree_registry_path=str(tmp_path / "worktrees.json"),
            remote_config_path=str(remote_yml),
            roles_config_path=str(roles_yml),
        )
        remote_cfg = resolve_remote_config(config, "proj")
        assert remote_cfg.repo_url == "https://github.com/test.git"
        assert remote_cfg.git.token_env == "GIT_TOKEN"
        assert remote_cfg.provisioner.type == "manual"
        assert "claude" in remote_cfg.required_clis

    def test_resolve_remote_config_missing_path(self, tmp_path: Path):
        config = _make_config(tmp_path)
        with pytest.raises(ValueError, match="WM_REMOTE_CONFIG"):
            resolve_remote_config(config, "proj")

    def test_resolve_agent_tool_gate(self, tmp_path: Path):
        config = _make_config(tmp_path)
        tool = resolve_agent_tool(config, Phase.GATE)
        assert tool.cli == Cli.BASH

    def test_resolve_required_secrets_with_mcp(self):
        from tanren_core.env.environment_schema import (
            DispatchProvisionerConfig,
            EnvironmentProfile,
            EnvironmentProfileType,
            McpServerConfig,
            RemoteExecutionConfig,
        )

        profile = EnvironmentProfile(
            name="prod",
            type=EnvironmentProfileType.REMOTE,
            remote_config=RemoteExecutionConfig(
                provisioner=DispatchProvisionerConfig(type="manual", settings={}),
                repo_url="https://github.com/test.git",
                required_clis=("codex",),
            ),
            mcp={
                "ctx7": McpServerConfig(
                    url="https://mcp.example.com",
                    headers={"x-api-key": "$MCP_CTX7_KEY"},
                )
            },
        )
        secrets = resolve_required_secrets(profile)
        assert "CODEX_AUTH_JSON" in secrets
        assert "MCP_CTX7_KEY" in secrets

    @pytest.mark.asyncio
    async def test_resolve_cloud_secrets_no_tanren_yml(self, tmp_path: Path):
        config = _make_config(tmp_path)
        (Path(config.github_dir) / "proj").mkdir()
        result = await resolve_cloud_secrets(config, "proj")
        assert result == {}

    def test_resolve_agent_tool_do_task_needs_roles(self, tmp_path: Path):
        config = _make_config(tmp_path)
        with pytest.raises(ValueError, match="WM_ROLES_CONFIG_PATH"):
            resolve_agent_tool(config, Phase.DO_TASK)

    def test_resolve_gate_cmd_provided(self, tmp_path: Path):
        config = _make_config(tmp_path)
        assert resolve_gate_cmd(config, "p", "d", Phase.GATE, "make test") == "make test"

    def test_resolve_gate_cmd_non_gate(self, tmp_path: Path):
        config = _make_config(tmp_path)
        assert resolve_gate_cmd(config, "p", "d", Phase.DO_TASK, None) is None

    @pytest.mark.asyncio
    async def test_resolve_cloud_secrets_no_sources(self, tmp_path: Path):
        config = _make_config(tmp_path)
        project_dir = Path(config.github_dir) / "proj"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text(
            "env:\n  required:\n    - key: FOO\n      description: test\n"
        )
        result = await resolve_cloud_secrets(config, "proj")
        assert result == {}
