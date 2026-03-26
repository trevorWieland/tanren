"""Tests for Ubuntu VM bootstrapper."""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.remote_types import RemoteResult
from tanren_core.adapters.ubuntu_bootstrap import (
    _AGENT_USER,
    _MARKER_PATH,
    UbuntuBootstrapper,
)
from tanren_core.schemas import Cli


def _ok(stdout: str = "") -> RemoteResult:
    return RemoteResult(exit_code=0, stdout=stdout, stderr="", timed_out=False)


def _fail(stderr: str = "error") -> RemoteResult:
    return RemoteResult(exit_code=1, stdout="", stderr=stderr, timed_out=False)


def _make_conn(
    *, marker_exists: bool = False, installed_tools: frozenset[str] = frozenset()
) -> AsyncMock:
    """Create a mock RemoteConnection.

    Args:
        marker_exists: If True, the marker-file check returns "exists".
        installed_tools: Set of tool names whose ``command -v`` check succeeds.
    """
    conn = AsyncMock()

    async def _run(cmd: str, **kwargs) -> RemoteResult:  # noqa: RUF029 — async required by interface
        # Marker check
        if _MARKER_PATH in cmd and "test -f" in cmd:
            return _ok("exists") if marker_exists else _ok("")
        # Tool presence checks (command -v or npx ... --version)
        if cmd.startswith("command -v "):
            tool = cmd.rsplit(maxsplit=1)[-1]
            if tool in installed_tools:
                return _ok(f"/usr/bin/{tool}")
            return _fail("not found")
        if cmd.startswith("npx ") and "--version" in cmd:
            return _fail("not found")
        # Everything else succeeds
        return _ok()

    conn.run = AsyncMock(side_effect=_run)
    conn.upload_content = AsyncMock()
    return conn


class TestFreshBootstrap:
    @pytest.mark.asyncio
    async def test_installs_all_tools_and_writes_marker(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE, Cli.OPENCODE}))

        result = await bs.bootstrap(conn)

        assert "apt-packages" in result.installed
        assert "docker" in result.installed
        assert "node" in result.installed
        assert "uv" in result.installed
        assert "claude" in result.installed
        assert result.skipped == ()
        # Marker file must be written (touch command)
        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert any(f"touch {_MARKER_PATH}" in c for c in run_cmds)


class TestIdempotent:
    @pytest.mark.asyncio
    async def test_marker_exists_skips_all(self):
        conn = _make_conn(marker_exists=True)
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        result = await bs.bootstrap(conn)

        assert result.duration_secs == 0
        assert result.installed == ()
        assert result.skipped == ()
        # Only one call: the marker check
        assert conn.run.call_count == 1


class TestSkipInstalledTools:
    @pytest.mark.asyncio
    async def test_already_installed_tools_are_skipped(self):
        conn = _make_conn(installed_tools=frozenset({"docker", "uv"}))
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE, Cli.OPENCODE}))

        result = await bs.bootstrap(conn)

        assert "docker" in result.skipped
        assert "uv" in result.skipped
        assert "docker" not in result.installed
        assert "uv" not in result.installed
        # node and claude should still be installed
        assert "node" in result.installed
        assert "claude" in result.installed


class TestForceFlag:
    @pytest.mark.asyncio
    async def test_force_reruns_when_marker_exists(self):
        conn = _make_conn(marker_exists=True, installed_tools=frozenset({"docker"}))
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE, Cli.OPENCODE}))

        result = await bs.bootstrap(conn, force=True)

        # Force skips the marker check entirely, so all tools run.
        # docker check returns 0, but force=True means it still installs.
        assert "apt-packages" in result.installed
        assert "docker" in result.installed
        assert result.skipped == ()


class TestStepFailure:
    @pytest.mark.asyncio
    async def test_apt_failure_raises(self):
        conn = _make_conn()

        async def _run(cmd: str, **kwargs) -> RemoteResult:  # noqa: RUF029 — async required by interface
            if _MARKER_PATH in cmd and "test -f" in cmd:
                return _ok("")
            if "apt-get" in cmd:
                return _fail("apt broke")
            return _ok()

        conn.run = AsyncMock(side_effect=_run)
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        with pytest.raises(RuntimeError, match="apt install failed"):
            await bs.bootstrap(conn)

    @pytest.mark.asyncio
    async def test_tool_install_failure_raises(self):
        conn = _make_conn()

        async def _run(cmd: str, **kwargs) -> RemoteResult:  # noqa: RUF029 — async required by interface
            if _MARKER_PATH in cmd and "test -f" in cmd:
                return _ok("")
            if cmd.startswith("command -v"):
                return _fail("not found")
            if "get.docker.com" in cmd:
                return _fail("docker install failed")
            return _ok()

        conn.run = AsyncMock(side_effect=_run)
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        with pytest.raises(RuntimeError, match="Failed to install docker"):
            await bs.bootstrap(conn)


class TestExtraScript:
    @pytest.mark.asyncio
    async def test_extra_script_uploaded_and_executed(self):
        conn = _make_conn()
        script = "#!/bin/bash\necho hello"
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}), extra_script=script)

        result = await bs.bootstrap(conn)

        conn.upload_content.assert_awaited_once_with(script, "/tmp/tanren-extra-bootstrap.sh")
        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert any("bash /tmp/tanren-extra-bootstrap.sh" in c for c in run_cmds)
        assert "extra-script" in result.installed

    @pytest.mark.asyncio
    async def test_extra_script_failure_raises(self):
        conn = _make_conn()

        async def _run(cmd: str, **kwargs) -> RemoteResult:  # noqa: RUF029 — async required by interface
            if _MARKER_PATH in cmd and "test -f" in cmd:
                return _ok("")
            if cmd.startswith("command -v"):
                return _fail("not found")
            if "tanren-extra-bootstrap" in cmd:
                return _fail("script error")
            return _ok()

        conn.run = AsyncMock(side_effect=_run)
        bs = UbuntuBootstrapper(
            required_clis=frozenset({Cli.CLAUDE}),
            extra_script="#!/bin/bash\nexit 1",
        )

        with pytest.raises(RuntimeError, match="Extra bootstrap script failed"):
            await bs.bootstrap(conn)


class TestIsBootstrapped:
    @pytest.mark.asyncio
    async def test_returns_true_when_marker_exists(self):
        conn = _make_conn(marker_exists=True)
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        assert await bs.is_bootstrapped(conn) is True

    @pytest.mark.asyncio
    async def test_returns_false_when_no_marker(self):
        conn = _make_conn(marker_exists=False)
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        assert await bs.is_bootstrapped(conn) is False


class TestCodexInstalled:
    @pytest.mark.asyncio
    async def test_codex_installed_when_configured(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CODEX}))

        result = await bs.bootstrap(conn)

        assert "codex" in result.installed
        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert any("npm install -g @openai/codex" in c for c in run_cmds)

    @pytest.mark.asyncio
    async def test_codex_not_installed_when_not_configured(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        result = await bs.bootstrap(conn)

        assert "codex" not in result.installed
        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert not any("@openai/codex" in c for c in run_cmds)


class TestCcusageAdaptsToConfiguredClis:
    @pytest.mark.asyncio
    async def test_ccusage_only_claude_packages(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        result = await bs.bootstrap(conn)

        assert "ccusage" in result.installed
        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        install_cmds = [c for c in run_cmds if "npm install -g" in c and "ccusage" in c]
        assert len(install_cmds) == 1
        assert "ccusage" in install_cmds[0]
        assert "@ccusage/codex" not in install_cmds[0]

    @pytest.mark.asyncio
    async def test_ccusage_all_cli_packages(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE, Cli.CODEX, Cli.OPENCODE}))

        result = await bs.bootstrap(conn)

        assert "ccusage" in result.installed
        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        install_cmds = [c for c in run_cmds if "npm install -g" in c and "ccusage" in c]
        assert len(install_cmds) == 1
        assert "ccusage" in install_cmds[0]
        assert "@ccusage/codex" in install_cmds[0]
        assert "@ccusage/opencode" in install_cmds[0]


class TestAgentUserCreation:
    @pytest.mark.asyncio
    async def test_creates_tanren_user_and_chowns_workspace(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        await bs.bootstrap(conn)

        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert any("useradd" in c and _AGENT_USER in c for c in run_cmds)
        assert any(f"chown {_AGENT_USER}:{_AGENT_USER} /workspace" in c for c in run_cmds)


class TestClaudeOnboardingFlag:
    @pytest.mark.asyncio
    async def test_creates_claude_json_when_claude_required(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        await bs.bootstrap(conn)

        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert any(".claude.json" in c and "hasCompletedOnboarding" in c for c in run_cmds)
        assert any(
            f"/home/{_AGENT_USER}/.claude.json" in c and "hasCompletedOnboarding" in c
            for c in run_cmds
        )

    @pytest.mark.asyncio
    async def test_no_claude_json_when_claude_not_required(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CODEX}))

        await bs.bootstrap(conn)

        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert not any(".claude.json" in c for c in run_cmds)


class TestDockerDaemonSetup:
    @pytest.mark.asyncio
    async def test_docker_install_enables_and_starts_daemon(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        await bs.bootstrap(conn)

        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        docker_install = [c for c in run_cmds if "get.docker.com" in c]
        assert len(docker_install) == 1
        assert "systemctl enable --now docker" in docker_install[0]

    @pytest.mark.asyncio
    async def test_docker_install_verifies_responsiveness(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        await bs.bootstrap(conn)

        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        docker_install = [c for c in run_cmds if "get.docker.com" in c]
        assert len(docker_install) == 1
        assert "docker info" in docker_install[0]

    @pytest.mark.asyncio
    async def test_agent_user_added_to_docker_group(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))

        await bs.bootstrap(conn)

        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert any(f"usermod -aG docker {_AGENT_USER}" in c for c in run_cmds)

    @pytest.mark.asyncio
    async def test_docker_group_skipped_when_docker_infra_skipped(self):
        conn = _make_conn()
        bs = UbuntuBootstrapper(
            required_clis=frozenset({Cli.CLAUDE}),
            skip_infra_tools=frozenset({"docker"}),
        )

        await bs.bootstrap(conn)

        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert not any("usermod -aG docker" in c for c in run_cmds)


class TestPlan:
    def test_plan_returns_only_configured_clis(self):
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE}))
        plan = bs.plan()

        step_names = [s.name for s in plan.install_steps]
        assert "claude" in step_names
        assert "opencode" not in step_names
        assert "codex" not in step_names
        assert "ccusage" in step_names
        # Infra always present
        assert "docker" in step_names
        assert "node" in step_names
        assert "uv" in step_names

    def test_plan_includes_codex_when_configured(self):
        bs = UbuntuBootstrapper(required_clis=frozenset({Cli.CLAUDE, Cli.CODEX}))
        plan = bs.plan()

        step_names = [s.name for s in plan.install_steps]
        assert "claude" in step_names
        assert "codex" in step_names
