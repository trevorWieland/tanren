"""Tests for Ubuntu VM bootstrapper."""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from worker_manager.adapters.remote_types import RemoteResult
from worker_manager.adapters.ubuntu_bootstrap import _MARKER_PATH, UbuntuBootstrapper


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

    async def _run(cmd: str, **kwargs) -> RemoteResult:
        # Marker check
        if _MARKER_PATH in cmd and "test -f" in cmd:
            return _ok("exists") if marker_exists else _ok("")
        # Tool presence checks
        if cmd.startswith("command -v "):
            tool = cmd.split()[-1]
            if tool in installed_tools:
                return _ok(f"/usr/bin/{tool}")
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
        bs = UbuntuBootstrapper()

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
        bs = UbuntuBootstrapper()

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
        bs = UbuntuBootstrapper()

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
        bs = UbuntuBootstrapper()

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

        async def _run(cmd: str, **kwargs) -> RemoteResult:
            if _MARKER_PATH in cmd and "test -f" in cmd:
                return _ok("")
            if "apt-get" in cmd:
                return _fail("apt broke")
            return _ok()

        conn.run = AsyncMock(side_effect=_run)
        bs = UbuntuBootstrapper()

        with pytest.raises(RuntimeError, match="apt install failed"):
            await bs.bootstrap(conn)

    @pytest.mark.asyncio
    async def test_tool_install_failure_raises(self):
        conn = _make_conn()

        async def _run(cmd: str, **kwargs) -> RemoteResult:
            if _MARKER_PATH in cmd and "test -f" in cmd:
                return _ok("")
            if cmd.startswith("command -v"):
                return _fail("not found")
            if "get.docker.com" in cmd:
                return _fail("docker install failed")
            return _ok()

        conn.run = AsyncMock(side_effect=_run)
        bs = UbuntuBootstrapper()

        with pytest.raises(RuntimeError, match="Failed to install docker"):
            await bs.bootstrap(conn)


class TestExtraScript:
    @pytest.mark.asyncio
    async def test_extra_script_uploaded_and_executed(self):
        conn = _make_conn()
        script = "#!/bin/bash\necho hello"
        bs = UbuntuBootstrapper(extra_script=script)

        result = await bs.bootstrap(conn)

        conn.upload_content.assert_awaited_once_with(script, "/tmp/tanren-extra-bootstrap.sh")
        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert any("bash /tmp/tanren-extra-bootstrap.sh" in c for c in run_cmds)
        assert "extra-script" in result.installed

    @pytest.mark.asyncio
    async def test_extra_script_failure_raises(self):
        conn = _make_conn()

        async def _run(cmd: str, **kwargs) -> RemoteResult:
            if _MARKER_PATH in cmd and "test -f" in cmd:
                return _ok("")
            if cmd.startswith("command -v"):
                return _fail("not found")
            if "tanren-extra-bootstrap" in cmd:
                return _fail("script error")
            return _ok()

        conn.run = AsyncMock(side_effect=_run)
        bs = UbuntuBootstrapper(extra_script="#!/bin/bash\nexit 1")

        with pytest.raises(RuntimeError, match="Extra bootstrap script failed"):
            await bs.bootstrap(conn)


class TestIsBootstrapped:
    @pytest.mark.asyncio
    async def test_returns_true_when_marker_exists(self):
        conn = _make_conn(marker_exists=True)
        bs = UbuntuBootstrapper()

        assert await bs.is_bootstrapped(conn) is True

    @pytest.mark.asyncio
    async def test_returns_false_when_no_marker(self):
        conn = _make_conn(marker_exists=False)
        bs = UbuntuBootstrapper()

        assert await bs.is_bootstrapped(conn) is False
