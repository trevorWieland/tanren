"""SSH connection adapter using paramiko."""

from __future__ import annotations

import asyncio
import logging
import time
from dataclasses import dataclass
from pathlib import Path

import paramiko

from worker_manager.adapters.remote_types import RemoteResult

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class SSHConfig:
    """SSH connection configuration."""

    host: str
    user: str = "root"
    key_path: str = "~/.ssh/tanren_vm"
    port: int = 22
    connect_timeout: int = 10


class SSHConnection:
    """Paramiko-based SSH connection implementing the RemoteConnection protocol.

    All paramiko calls are wrapped in asyncio.to_thread() to avoid blocking
    the event loop. The connection is independent of the workspace — if the
    agent deletes /workspace, SSH still works.
    """

    def __init__(self, config: SSHConfig) -> None:
        self._config = config
        self._client: paramiko.SSHClient | None = None
        self._sftp: paramiko.SFTPClient | None = None

    def _ensure_connected(self) -> paramiko.SSHClient:
        """Lazy connect with auto-reconnect on dead transport."""
        if self._client is not None:
            transport = self._client.get_transport()
            if transport is not None and transport.is_active():
                return self._client
            # Transport is dead — reconnect
            logger.info("SSH transport dead, reconnecting to %s", self._config.host)
            self._close_sync()

        client = paramiko.SSHClient()
        client.set_missing_host_key_policy(paramiko.AutoAddPolicy())

        key_path = str(Path(self._config.key_path).expanduser())
        try:
            client.connect(
                hostname=self._config.host,
                port=self._config.port,
                username=self._config.user,
                key_filename=key_path,
                timeout=self._config.connect_timeout,
            )
        except paramiko.AuthenticationException as e:
            raise ConnectionError(
                f"SSH auth failed for {self._config.user}@{self._config.host}: {e}"
            ) from e
        except paramiko.SSHException as e:
            raise ConnectionError(
                f"SSH connection failed to {self._config.host}: {e}"
            ) from e

        self._client = client
        self._sftp = None  # Reset SFTP on reconnect
        logger.info(
            "SSH connected to %s@%s:%d",
            self._config.user, self._config.host, self._config.port,
        )
        return client

    def _get_sftp(self) -> paramiko.SFTPClient:
        """Get or create SFTP client."""
        if self._sftp is not None:
            try:
                self._sftp.stat(".")
                return self._sftp
            except Exception:
                self._sftp = None

        client = self._ensure_connected()
        self._sftp = client.open_sftp()
        return self._sftp

    def _run_sync(
        self,
        command: str,
        *,
        timeout: int | None = None,
        stdin_data: str | None = None,
    ) -> RemoteResult:
        """Execute a command synchronously (called via to_thread)."""
        client = self._ensure_connected()
        channel = client.get_transport().open_session()

        if timeout is not None:
            channel.settimeout(float(timeout))

        channel.exec_command(command)

        if stdin_data is not None:
            channel.sendall(stdin_data.encode())
            channel.shutdown_write()

        timed_out = False
        try:
            stdout_data = b""
            stderr_data = b""

            while True:
                if channel.exit_status_ready():
                    # Drain remaining data
                    while channel.recv_ready():
                        stdout_data += channel.recv(65536)
                    while channel.recv_stderr_ready():
                        stderr_data += channel.recv_stderr(65536)
                    break

                if channel.recv_ready():
                    stdout_data += channel.recv(65536)
                if channel.recv_stderr_ready():
                    stderr_data += channel.recv_stderr(65536)
                if not channel.recv_ready() and not channel.recv_stderr_ready():
                    time.sleep(0.05)

            exit_code = channel.recv_exit_status()
        except TimeoutError:
            timed_out = True
            exit_code = -1
            stdout_data = b""
            stderr_data = b"Command timed out"
        finally:
            channel.close()

        return RemoteResult(
            exit_code=exit_code,
            stdout=stdout_data.decode(errors="replace"),
            stderr=stderr_data.decode(errors="replace"),
            timed_out=timed_out,
        )

    async def run(
        self,
        command: str,
        *,
        timeout: int | None = None,
        stdin_data: str | None = None,
    ) -> RemoteResult:
        """Execute a command on the remote host."""
        return await asyncio.to_thread(
            self._run_sync, command, timeout=timeout, stdin_data=stdin_data
        )

    async def run_script(self, script: str, *, timeout: int | None = None) -> RemoteResult:
        """Execute a bash script via stdin."""
        return await self.run("bash -s", timeout=timeout, stdin_data=script)

    def _upload_sync(self, content: str, remote_path: str) -> None:
        """Upload content to remote path synchronously."""
        sftp = self._get_sftp()
        with sftp.file(remote_path, "w") as f:
            f.write(content)

    async def upload_content(self, content: str, remote_path: str) -> None:
        """Upload string content to a remote file via SFTP."""
        await asyncio.to_thread(self._upload_sync, content, remote_path)

    def _download_sync(self, remote_path: str) -> str | None:
        """Download content from remote path synchronously. Returns None if missing."""
        sftp = self._get_sftp()
        try:
            with sftp.file(remote_path, "r") as f:
                return f.read().decode(errors="replace")
        except FileNotFoundError:
            return None
        except OSError:
            return None

    async def download_content(self, remote_path: str) -> str | None:
        """Download string content from a remote file. Returns None if file missing."""
        return await asyncio.to_thread(self._download_sync, remote_path)

    async def check_connection(self) -> bool:
        """Test connectivity with a simple echo command."""
        try:
            result = await self.run("echo tanren-ok", timeout=10)
            return result.exit_code == 0 and "tanren-ok" in result.stdout
        except Exception:
            return False

    def get_host_identifier(self) -> str:
        """Return the host identifier for this connection."""
        return f"{self._config.user}@{self._config.host}:{self._config.port}"

    def _close_sync(self) -> None:
        """Close connection synchronously."""
        import contextlib

        if self._sftp is not None:
            with contextlib.suppress(Exception):
                self._sftp.close()
            self._sftp = None
        if self._client is not None:
            with contextlib.suppress(Exception):
                self._client.close()
            self._client = None

    async def close(self) -> None:
        """Close SSH and SFTP connections."""
        await asyncio.to_thread(self._close_sync)
