"""Integration tests for ubuntu_bootstrap.py — UbuntuBootstrapper with mocked SSH."""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.remote_types import RemoteResult
from tanren_core.adapters.ubuntu_bootstrap import (
    _AGENT_USER,  # noqa: PLC2701
    _MARKER_PATH,  # noqa: PLC2701
    UbuntuBootstrapper,
)
from tanren_core.schemas import Cli

# Default required CLIs for integration tests
_DEFAULT_CLIS = frozenset({Cli.CLAUDE, Cli.OPENCODE, Cli.CODEX})

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_conn(
    *,
    marker_exists: bool = False,
    all_tools_installed: bool = False,
    fail_step: str | None = None,
    extra_script_fails: bool = False,
) -> AsyncMock:
    """Build a mock RemoteConnection with configurable behaviour.

    Args:
        marker_exists: If True, the marker-file check returns "exists".
        all_tools_installed: If True, every tool check returns exit_code=0.
        fail_step: Name of a bootstrap step whose install should fail.
        extra_script_fails: If True, running the extra script returns non-zero.
    """
    conn = AsyncMock()

    def _run_side_effect(
        command: str,
        *,
        timeout: int | None = None,
        stdin_data: str | None = None,
    ) -> RemoteResult:
        # Marker check
        if f"test -f {_MARKER_PATH}" in command:
            if marker_exists:
                return RemoteResult(exit_code=0, stdout="exists")
            return RemoteResult(exit_code=1, stdout="")

        # apt-get update && install
        if "apt-get" in command and "install" in command:
            return RemoteResult(exit_code=0, stdout="")

        # Tool check commands (command -v <tool> or npx ... --version)
        if command.startswith("command -v"):
            if all_tools_installed:
                return RemoteResult(exit_code=0, stdout="/usr/bin/tool")
            return RemoteResult(exit_code=1, stdout="")
        if command.startswith("npx ") and "--version" in command:
            if all_tools_installed:
                return RemoteResult(exit_code=0, stdout="1.0.0")
            return RemoteResult(exit_code=1, stdout="")

        # Tool install commands — check for known install patterns
        if fail_step:
            # Match by step name in install command
            step_patterns = {
                "docker": "get.docker.com",
                "node": "nodesource.com",
                "uv": "astral.sh",
                "claude": "@anthropic-ai/claude-code",
                "opencode": "opencode.ai/install",
                "codex": "@openai/codex",
                "ccusage": "ccusage",
            }
            pattern = step_patterns.get(fail_step, "")
            if pattern and pattern in command:
                return RemoteResult(exit_code=1, stdout="", stderr=f"{fail_step} install failed")

        # mkdir, chown, useradd, etc.
        if (
            "mkdir -p" in command
            or "chown" in command
            or "useradd" in command
            or "id -u" in command
        ):
            return RemoteResult(exit_code=0, stdout="")

        # touch marker
        if f"touch {_MARKER_PATH}" in command:
            return RemoteResult(exit_code=0, stdout="")

        # extra bootstrap script
        if "tanren-extra-bootstrap.sh" in command:
            if extra_script_fails:
                return RemoteResult(exit_code=1, stdout="", stderr="script error")
            return RemoteResult(exit_code=0, stdout="")

        # Fallback
        return RemoteResult(exit_code=0, stdout="")

    conn.run = AsyncMock(side_effect=_run_side_effect)
    conn.upload_content = AsyncMock()
    return conn


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestBootstrapRunsExpectedSteps:
    async def test_all_steps_executed(self) -> None:
        """Full bootstrap installs apt packages, all tools, creates workspace, writes marker."""
        conn = _make_conn()
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        result = await bootstrapper.bootstrap(conn)

        # Verify apt install was called (match only the apt-get update && install line)
        apt_calls = [c for c in conn.run.call_args_list if c[0][0].startswith("apt-get update")]
        assert len(apt_calls) == 1

        # Verify each infra tool was installed
        assert "docker" in result.installed
        assert "node" in result.installed
        assert "uv" in result.installed

        # Verify CLI tools installed
        assert "claude" in result.installed
        assert "opencode" in result.installed
        assert "codex" in result.installed

        # Verify ccusage installed
        assert "ccusage" in result.installed

        # Verify agent user created
        run_cmds = [c[0][0] for c in conn.run.call_args_list]
        assert any("useradd" in c and _AGENT_USER in c for c in run_cmds)

        # Verify workspace dir created with chown
        assert any(f"chown {_AGENT_USER}:{_AGENT_USER} /workspace" in c for c in run_cmds)

        # Verify marker written
        marker_calls = [c for c in conn.run.call_args_list if f"touch {_MARKER_PATH}" in c[0][0]]
        assert len(marker_calls) == 1

        # Verify result
        assert "apt-packages" in result.installed


class TestBootstrapSkipsWhenMarkerExists:
    async def test_skips_on_marker(self) -> None:
        """When marker file exists and force=False, bootstrap returns immediately."""
        conn = _make_conn(marker_exists=True)
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        result = await bootstrapper.bootstrap(conn)

        assert result.duration_secs == 0
        assert result.installed == ()
        # Should only have the marker check call
        assert conn.run.call_count == 1


class TestBootstrapWithExtraScript:
    async def test_extra_script_executed(self) -> None:
        """When extra_script is provided, it should be uploaded and run."""
        conn = _make_conn()
        script_content = "#!/bin/bash\necho 'custom setup'"
        bootstrapper = UbuntuBootstrapper(
            required_clis=_DEFAULT_CLIS,
            extra_script=script_content,
        )

        result = await bootstrapper.bootstrap(conn)

        # Verify upload_content was called with the script
        conn.upload_content.assert_awaited_once_with(
            script_content, "/tmp/tanren-extra-bootstrap.sh"
        )

        # Verify the script was executed
        script_run_calls = [
            c for c in conn.run.call_args_list if "tanren-extra-bootstrap.sh" in c[0][0]
        ]
        assert len(script_run_calls) == 1

        assert "extra-script" in result.installed


class TestBootstrapForceFlag:
    async def test_force_ignores_marker(self) -> None:
        """force=True should run bootstrap even when marker exists."""
        conn = _make_conn(marker_exists=True)
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        result = await bootstrapper.bootstrap(conn, force=True)

        # When force=True the marker check is skipped entirely
        marker_check_calls = [
            c for c in conn.run.call_args_list if f"test -f {_MARKER_PATH}" in c[0][0]
        ]
        assert len(marker_check_calls) == 0

        # Should have installed everything
        assert "apt-packages" in result.installed

    async def test_force_reinstalls_existing_tools(self) -> None:
        """force=True should install tools even when check passes."""
        conn = _make_conn(all_tools_installed=True)
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        result = await bootstrapper.bootstrap(conn, force=True)

        # All infra and cli tools should be installed despite being present
        assert "docker" in result.installed
        assert "claude" in result.installed
        assert "codex" in result.installed
        assert result.skipped == ()


class TestBootstrapStepFailure:
    async def test_apt_failure_raises(self) -> None:
        """apt install failure should raise RuntimeError."""
        conn = AsyncMock()
        # Marker check: not exists
        conn.run = AsyncMock(
            side_effect=[
                RemoteResult(exit_code=1, stdout=""),  # marker check
                RemoteResult(exit_code=1, stdout="", stderr="apt broken"),  # apt install
            ]
        )
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        with pytest.raises(RuntimeError, match="apt install failed"):
            await bootstrapper.bootstrap(conn)

    async def test_tool_install_failure_raises(self) -> None:
        """Individual tool install failure should raise RuntimeError."""
        conn = _make_conn(fail_step="docker")
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        with pytest.raises(RuntimeError, match="Failed to install docker"):
            await bootstrapper.bootstrap(conn)

    async def test_extra_script_failure_raises(self) -> None:
        """Extra script failure should raise RuntimeError."""
        conn = _make_conn(extra_script_fails=True)
        bootstrapper = UbuntuBootstrapper(
            required_clis=_DEFAULT_CLIS,
            extra_script="#!/bin/bash\nexit 1",
        )

        with pytest.raises(RuntimeError, match="Extra bootstrap script failed"):
            await bootstrapper.bootstrap(conn)


class TestBootstrapCreatesWorkspaceDir:
    async def test_mkdir_workspace_called(self) -> None:
        """Bootstrap should always create /workspace directory."""
        conn = _make_conn()
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        await bootstrapper.bootstrap(conn)

        mkdir_calls = [c for c in conn.run.call_args_list if "mkdir -p /workspace" in c[0][0]]
        assert len(mkdir_calls) >= 1


class TestIsBootstrapped:
    async def test_returns_true_when_marker_exists(self) -> None:
        conn = AsyncMock()
        conn.run = AsyncMock(return_value=RemoteResult(exit_code=0, stdout="exists"))
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        assert await bootstrapper.is_bootstrapped(conn) is True

    async def test_returns_false_when_no_marker(self) -> None:
        conn = AsyncMock()
        conn.run = AsyncMock(return_value=RemoteResult(exit_code=1, stdout=""))
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        assert await bootstrapper.is_bootstrapped(conn) is False


class TestBootstrapPlan:
    def test_plan_returns_expected_structure(self) -> None:
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)
        plan = bootstrapper.plan()

        step_names = [s.name for s in plan.install_steps]
        assert "docker" in step_names
        assert "node" in step_names
        assert "uv" in step_names
        assert "claude" in step_names
        assert "opencode" in step_names
        assert "codex" in step_names
        assert "ccusage" in step_names


class TestBootstrapSkipsInstalledTools:
    async def test_skips_already_installed(self) -> None:
        """When all tools pass the check command, they should be skipped (not force)."""
        conn = _make_conn(all_tools_installed=True)
        bootstrapper = UbuntuBootstrapper(required_clis=_DEFAULT_CLIS)

        result = await bootstrapper.bootstrap(conn)

        assert "docker" in result.skipped
        assert "claude" in result.skipped
        assert "codex" in result.skipped
        # Only apt-packages should be in installed
        assert result.installed == ("apt-packages",)
