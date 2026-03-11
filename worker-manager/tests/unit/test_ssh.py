"""Tests for SSH connection adapter."""

from __future__ import annotations

from unittest.mock import MagicMock, patch

import pytest
from pydantic import ValidationError

from worker_manager.adapters.remote_types import RemoteResult
from worker_manager.adapters.ssh import SSHConfig, SSHConnection

# ---------------------------------------------------------------------------
# SSHConfig defaults
# ---------------------------------------------------------------------------


class TestSSHConfig:
    def test_defaults(self):
        cfg = SSHConfig(host="10.0.0.1")
        assert cfg.host == "10.0.0.1"
        assert cfg.user == "root"
        assert cfg.key_path == "~/.ssh/tanren_vm"
        assert cfg.port == 22
        assert cfg.connect_timeout == 10

    def test_frozen(self):
        cfg = SSHConfig(host="10.0.0.1")
        with pytest.raises(ValidationError, match="Instance is frozen"):
            cfg.host = "other"  # type: ignore[misc]


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_conn(host: str = "10.0.0.1", **kw) -> SSHConnection:
    return SSHConnection(SSHConfig(host=host, **kw))


def _mock_channel(
    stdout: bytes = b"",
    stderr: bytes = b"",
    exit_code: int = 0,
) -> MagicMock:
    """Return a mock paramiko Channel with sensible read behaviour."""
    chan = MagicMock()

    # First call to recv_ready/recv_stderr_ready returns True (data available),
    # subsequent calls during drain return False.
    stdout_calls = iter([True, True, False])
    stderr_calls = iter([True, True, False])

    chan.exit_status_ready.side_effect = [False, True]
    chan.recv_ready.side_effect = lambda: next(stdout_calls, False)
    chan.recv_stderr_ready.side_effect = lambda: next(stderr_calls, False)
    chan.recv.return_value = stdout
    chan.recv_stderr.return_value = stderr
    chan.recv_exit_status.return_value = exit_code
    return chan


# ---------------------------------------------------------------------------
# SSHConnection._ensure_connected
# ---------------------------------------------------------------------------


@patch("worker_manager.adapters.ssh.paramiko")
class TestEnsureConnected:
    def test_creates_client_and_connects(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client

        conn = _make_conn(user="deploy", port=2222, connect_timeout=5)
        result = conn._ensure_connected()

        assert result is mock_client
        mock_client.set_missing_host_key_policy.assert_called_once_with(
            mock_paramiko.AutoAddPolicy()
        )
        mock_client.connect.assert_called_once()
        call_kwargs = mock_client.connect.call_args.kwargs
        assert call_kwargs["hostname"] == "10.0.0.1"
        assert call_kwargs["port"] == 2222
        assert call_kwargs["username"] == "deploy"
        assert call_kwargs["timeout"] == 5

    def test_reuses_active_transport(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        transport = MagicMock()
        transport.is_active.return_value = True
        mock_client.get_transport.return_value = transport

        conn = _make_conn()
        conn._ensure_connected()
        conn._ensure_connected()

        # SSHClient constructed only once
        assert mock_paramiko.SSHClient.call_count == 1

    def test_reconnects_on_dead_transport(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        transport = MagicMock()
        transport.is_active.return_value = False
        mock_client.get_transport.return_value = transport

        conn = _make_conn()
        conn._ensure_connected()
        conn._ensure_connected()

        # Should create a new client on the second call
        assert mock_paramiko.SSHClient.call_count == 2

    def test_auth_exception_wraps_to_connection_error(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        mock_paramiko.AuthenticationException = type("AuthenticationException", (Exception,), {})
        mock_client.connect.side_effect = mock_paramiko.AuthenticationException("bad key")

        conn = _make_conn()
        with pytest.raises(ConnectionError, match="SSH auth failed"):
            conn._ensure_connected()

    def test_ssh_exception_wraps_to_connection_error(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        mock_paramiko.SSHException = type("SSHException", (Exception,), {})
        mock_paramiko.AuthenticationException = type(
            "AuthenticationException", (mock_paramiko.SSHException,), {}
        )
        mock_client.connect.side_effect = mock_paramiko.SSHException("network")

        conn = _make_conn()
        with pytest.raises(ConnectionError, match="SSH connection failed"):
            conn._ensure_connected()


# ---------------------------------------------------------------------------
# SSHConnection._run_sync / run
# ---------------------------------------------------------------------------


@patch("worker_manager.adapters.ssh.paramiko")
class TestRunSync:
    def test_returns_remote_result(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        chan = _mock_channel(stdout=b"hello\n", exit_code=0)
        mock_client.get_transport.return_value.open_session.return_value = chan

        conn = _make_conn()
        result = conn._run_sync("echo hello")

        assert isinstance(result, RemoteResult)
        assert result.exit_code == 0
        assert "hello" in result.stdout
        assert result.timed_out is False
        chan.exec_command.assert_called_once_with("echo hello")

    def test_timeout_sets_timed_out(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        chan = MagicMock()
        mock_client.get_transport.return_value.open_session.return_value = chan
        chan.exit_status_ready.side_effect = TimeoutError("timed out")

        conn = _make_conn()
        result = conn._run_sync("sleep 999", timeout=1)

        assert result.timed_out is True
        assert result.exit_code == -1
        assert "timed out" in result.stderr.lower()
        chan.settimeout.assert_called_once_with(1.0)

    @patch("worker_manager.adapters.ssh.time")
    def test_wall_clock_timeout_catches_silent_hang(self, mock_time, mock_paramiko):
        """Wall-clock timeout fires when command hangs without producing output."""
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        chan = MagicMock()
        mock_client.get_transport.return_value.open_session.return_value = chan

        # Channel never becomes ready — simulates a hung command
        chan.exit_status_ready.return_value = False
        chan.recv_ready.return_value = False
        chan.recv_stderr_ready.return_value = False

        # Simulate time advancing past the timeout on the second monotonic() call
        mock_time.monotonic.side_effect = [0.0, 0.0, 11.0]
        mock_time.sleep = MagicMock()

        conn = _make_conn()
        result = conn._run_sync("hang-forever", timeout=10)

        assert result.timed_out is True
        assert result.exit_code == -1
        assert "timed out" in result.stderr.lower()

    def test_stdin_data_sent_via_channel(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        chan = _mock_channel(exit_code=0)
        mock_client.get_transport.return_value.open_session.return_value = chan

        conn = _make_conn()
        conn._run_sync("cat", stdin_data="payload")

        chan.sendall.assert_called_once_with(b"payload")
        chan.shutdown_write.assert_called_once()

    @patch("worker_manager.adapters.ssh.time")
    def test_sleeps_when_no_data_available(self, mock_time, mock_paramiko):
        """Verify sleep is called when neither stdout nor stderr has data."""
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client

        chan = MagicMock()
        mock_client.get_transport.return_value.open_session.return_value = chan

        # Loop: first iteration no data (sleep), second iteration exit ready
        chan.exit_status_ready.side_effect = [False, True]
        chan.recv_ready.side_effect = [False, False, False]  # loop check, sleep guard, drain
        chan.recv_stderr_ready.side_effect = [False, False, False]  # loop check, sleep guard, drain
        chan.recv_exit_status.return_value = 0

        conn = _make_conn()
        result = conn._run_sync("echo test")

        mock_time.sleep.assert_called_with(0.05)
        assert result.exit_code == 0


# ---------------------------------------------------------------------------
# Async wrappers
# ---------------------------------------------------------------------------


@patch("worker_manager.adapters.ssh.paramiko")
class TestAsyncRun:
    @pytest.mark.asyncio
    async def test_run_delegates_to_run_sync(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        chan = _mock_channel(stdout=b"ok\n", exit_code=0)
        mock_client.get_transport.return_value.open_session.return_value = chan

        conn = _make_conn()
        result = await conn.run("echo ok")

        assert isinstance(result, RemoteResult)
        assert result.exit_code == 0

    @pytest.mark.asyncio
    async def test_run_script_passes_stdin(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        chan = _mock_channel(exit_code=0)
        mock_client.get_transport.return_value.open_session.return_value = chan

        conn = _make_conn()
        await conn.run_script("set -e\necho done")

        chan.exec_command.assert_called_once_with("bash -s")
        chan.sendall.assert_called_once_with(b"set -e\necho done")


# ---------------------------------------------------------------------------
# SFTP upload / download
# ---------------------------------------------------------------------------


@patch("worker_manager.adapters.ssh.paramiko")
class TestSFTP:
    @pytest.mark.asyncio
    async def test_upload_content_writes_file(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        mock_sftp = MagicMock()
        mock_client.open_sftp.return_value = mock_sftp
        mock_file = MagicMock()
        mock_sftp.file.return_value.__enter__ = MagicMock(return_value=mock_file)
        mock_sftp.file.return_value.__exit__ = MagicMock(return_value=False)

        conn = _make_conn()
        await conn.upload_content("file body", "/tmp/test.txt")

        mock_sftp.file.assert_called_once_with("/tmp/test.txt", "w")
        mock_file.write.assert_called_once_with("file body")

    @pytest.mark.asyncio
    async def test_download_content_returns_string(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        mock_sftp = MagicMock()
        mock_client.open_sftp.return_value = mock_sftp
        mock_file = MagicMock()
        mock_file.read.return_value = b"remote data"
        mock_sftp.file.return_value.__enter__ = MagicMock(return_value=mock_file)
        mock_sftp.file.return_value.__exit__ = MagicMock(return_value=False)

        conn = _make_conn()
        content = await conn.download_content("/tmp/test.txt")

        assert content == "remote data"

    @pytest.mark.asyncio
    async def test_download_content_returns_none_for_missing(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        mock_sftp = MagicMock()
        mock_client.open_sftp.return_value = mock_sftp
        mock_sftp.file.side_effect = FileNotFoundError("no such file")

        conn = _make_conn()
        content = await conn.download_content("/tmp/missing.txt")

        assert content is None


# ---------------------------------------------------------------------------
# check_connection / get_host_identifier / close
# ---------------------------------------------------------------------------


@patch("worker_manager.adapters.ssh.paramiko")
class TestMisc:
    @pytest.mark.asyncio
    async def test_check_connection_true(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        chan = _mock_channel(stdout=b"tanren-ok\n", exit_code=0)
        mock_client.get_transport.return_value.open_session.return_value = chan

        conn = _make_conn()
        assert await conn.check_connection() is True

    @pytest.mark.asyncio
    async def test_check_connection_false_on_failure(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        mock_client.get_transport.side_effect = Exception("boom")

        conn = _make_conn()
        assert await conn.check_connection() is False

    def test_get_host_identifier(self, mock_paramiko):
        conn = _make_conn(user="deploy", port=2222)
        assert conn.get_host_identifier() == "deploy@10.0.0.1:2222"

    @pytest.mark.asyncio
    async def test_close_cleans_up(self, mock_paramiko):
        mock_client = MagicMock()
        mock_paramiko.SSHClient.return_value = mock_client
        mock_sftp = MagicMock()

        conn = _make_conn()
        conn._client = mock_client
        conn._sftp = mock_sftp

        await conn.close()

        mock_sftp.close.assert_called_once()
        mock_client.close.assert_called_once()
        assert conn._client is None
        assert conn._sftp is None
