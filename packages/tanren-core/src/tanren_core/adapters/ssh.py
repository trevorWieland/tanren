"""SSH connection adapter using paramiko."""

from __future__ import annotations

import asyncio
import contextlib
import logging
import socket
import time
from pathlib import Path
from typing import Literal

import paramiko
from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.remote_types import RemoteResult

logger = logging.getLogger(__name__)


class SSHConfig(BaseModel):
    """SSH connection configuration."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    host: str = Field(..., description="Remote host address (IP or hostname)")
    user: str = Field(default="root", description="SSH username")
    key_path: str = Field(
        default="~/.ssh/tanren_vm", description="Path to the SSH private key file"
    )
    port: int = Field(default=22, ge=1, le=65535, description="SSH port number")
    connect_timeout: int = Field(default=10, ge=1, description="Connection timeout in seconds")
    host_key_policy: Literal["auto_add", "warn", "reject"] = Field(
        default="auto_add", description="Host key verification policy"
    )


class SSHConnection:
    """Paramiko-based SSH connection implementing the RemoteConnection protocol.

    All paramiko calls are wrapped in asyncio.to_thread() to avoid blocking
    the event loop. The connection is independent of the workspace — if the
    agent deletes /workspace, SSH still works.
    """

    def __init__(self, config: SSHConfig) -> None:
        """Initialize with SSH connection configuration."""
        self._config = config
        self._client: paramiko.SSHClient | None = None
        self._sftp: paramiko.SFTPClient | None = None

    def _ensure_connected(self) -> paramiko.SSHClient:
        """Lazy connect with auto-reconnect on dead transport.

        Returns:
            Connected paramiko SSHClient.

        Raises:
            ConnectionError: If SSH authentication or connection fails.
        """
        if self._client is not None:
            transport = self._client.get_transport()
            if transport is not None and transport.is_active():
                return self._client
            # Transport is dead — reconnect
            logger.info("SSH transport dead, reconnecting to %s", self._config.host)
            self._close_sync()

        client = paramiko.SSHClient()

        if self._config.host_key_policy == "reject":
            client.load_system_host_keys()
            client.set_missing_host_key_policy(paramiko.RejectPolicy())
        elif self._config.host_key_policy == "warn":
            client.load_system_host_keys()
            client.set_missing_host_key_policy(paramiko.WarningPolicy())  # noqa: S507 — intentional AutoAddPolicy for ephemeral VMs
        else:
            # auto_add: skip system host keys — ephemeral VMs reuse IPs
            # and stale entries cause BadHostKeyException
            client.set_missing_host_key_policy(paramiko.AutoAddPolicy())  # noqa: S507 — intentional AutoAddPolicy for ephemeral VMs

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
            raise ConnectionError(f"SSH connection failed to {self._config.host}: {e}") from e
        except OSError as e:
            raise ConnectionError(f"SSH connection failed to {self._config.host}: {e}") from e

        self._client = client
        self._sftp = None  # Reset SFTP on reconnect
        logger.info(
            "SSH connected to %s@%s:%d",
            self._config.user,
            self._config.host,
            self._config.port,
        )
        return client

    def _get_sftp(self) -> paramiko.SFTPClient:
        """Get or create SFTP client.

        Returns:
            Active SFTP client.
        """
        if self._sftp is not None:
            try:
                self._sftp.stat(".")
            except Exception:
                logger.debug(
                    "SFTP channel stale, recreating for %s", self._config.host, exc_info=True
                )
                self._sftp = None
            else:
                return self._sftp

        client = self._ensure_connected()
        self._sftp = client.open_sftp()
        return self._sftp

    def _run_sync(
        self,
        command: str,
        *,
        timeout: int | None = None,
        stdin_data: str | None = None,
        request_pty: bool = False,
    ) -> RemoteResult:
        """Execute a command synchronously (called via to_thread).

        Returns:
            RemoteResult with exit code, stdout, stderr, and timeout flag.

        Raises:
            TimeoutError: If the command exceeds the wall-clock timeout.
        """
        client = self._ensure_connected()
        channel = client.get_transport().open_session()

        if timeout is not None:
            channel.settimeout(float(timeout))

        if request_pty:
            channel.get_pty()

        channel.exec_command(command)

        if stdin_data is not None:
            channel.sendall(stdin_data.encode())
            channel.shutdown_write()

        timed_out = False
        start_time = time.monotonic()
        try:
            stdout_data = b""
            stderr_data = b""

            while True:
                # Wall-clock timeout check (catches silent hung commands)
                if timeout is not None and (time.monotonic() - start_time) > timeout:
                    raise TimeoutError("Command timed out")

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
        timeout_secs: int | None = None,
        stdin_data: str | None = None,
        request_pty: bool = False,
    ) -> RemoteResult:
        """Execute a command on the remote host.

        Returns:
            RemoteResult with exit code, stdout, stderr, and timeout flag.
        """
        return await asyncio.to_thread(
            self._run_sync,
            command,
            timeout=timeout_secs,
            stdin_data=stdin_data,
            request_pty=request_pty,
        )

    async def run_script(self, script: str, *, timeout_secs: int | None = None) -> RemoteResult:
        """Execute a bash script via stdin.

        Returns:
            RemoteResult from the script execution.
        """
        return await self.run("bash -s", timeout_secs=timeout_secs, stdin_data=script)

    def _upload_sync(self, content: str, remote_path: str) -> None:
        """Upload content to remote path synchronously."""
        sftp = self._get_sftp()
        with sftp.file(remote_path, "w") as f:
            f.write(content)

    async def upload_content(self, content: str, remote_path: str) -> None:
        """Upload string content to a remote file via SFTP."""
        await asyncio.to_thread(self._upload_sync, content, remote_path)

    def _download_sync(self, remote_path: str) -> str | None:
        """Download content from remote path synchronously.

        Returns:
            File content as string, or None if the file does not exist.
        """
        sftp = self._get_sftp()
        try:
            with sftp.file(remote_path, "r") as f:
                return f.read().decode(errors="replace")
        except FileNotFoundError:
            return None
        except OSError:
            return None

    async def download_content(self, remote_path: str) -> str | None:
        """Download string content from a remote file.

        Returns:
            File content as string, or None if the file does not exist.
        """
        return await asyncio.to_thread(self._download_sync, remote_path)

    def _check_ssh_banner_sync(self) -> bool:
        """Check if the remote SSH service sends a valid protocol banner.

        Uses a raw TCP socket to avoid paramiko transport thread side-effects.

        Returns:
            True if the SSH banner was received.
        """
        try:
            with socket.create_connection(
                (self._config.host, self._config.port),
                timeout=self._config.connect_timeout,
            ) as sock:
                data = sock.recv(256)
                return data.startswith(b"SSH-")
        except OSError as e:
            logger.debug(
                "SSH banner check failed for %s:%d: %s",
                self._config.host,
                self._config.port,
                e,
            )
            return False

    async def check_ssh_banner(self) -> bool:
        """Check if the remote SSH service is ready by reading the protocol banner.

        This is faster and more reliable than a full paramiko connection for
        readiness polling — it avoids spawning transport threads that can
        interfere with subsequent connection attempts.

        Returns:
            True if the SSH banner was received.
        """
        return await asyncio.to_thread(self._check_ssh_banner_sync)

    async def check_connection(self) -> bool:
        """Test connectivity with a simple echo command.

        Returns:
            True if the connection is alive.
        """
        try:
            result = await self.run("echo tanren-ok", timeout_secs=10)
        except Exception:
            logger.debug("check_connection failed for %s", self._config.host, exc_info=True)
            return False
        else:
            return result.exit_code == 0 and "tanren-ok" in result.stdout

    def get_host_identifier(self) -> str:
        """Return the host identifier for this connection.

        Returns:
            String in user@host:port format.
        """
        return f"{self._config.user}@{self._config.host}:{self._config.port}"

    def _close_sync(self) -> None:
        """Close connection synchronously."""
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
