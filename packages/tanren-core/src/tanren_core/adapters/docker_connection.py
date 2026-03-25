"""Docker container connection — execute commands and transfer files via docker exec."""

from __future__ import annotations

import asyncio
import base64
import contextlib
import logging
import shlex
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.remote_types import RemoteResult

if TYPE_CHECKING:
    import aiodocker
    from aiodocker.types import JSONObject

logger = logging.getLogger(__name__)


class DockerConfig(BaseModel):
    """Configuration for creating Docker containers."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    image: str = Field(default="ubuntu:24.04", description="Docker image to use for containers")
    socket_url: str | None = Field(
        default=None, description="Docker daemon socket URL (None for default)"
    )
    network: str | None = Field(
        default=None, description="Docker network to attach the container to"
    )
    extra_volumes: tuple[str, ...] = Field(
        default=(), description="Additional bind-mount volume specs (host:container format)"
    )
    extra_env: dict[str, str] = Field(
        default_factory=dict, description="Extra environment variables for the container"
    )
    api_version: str = Field(default="v1.45", description="Docker API version string")


class DockerConnection:
    """Execute commands inside a Docker container via aiodocker exec.

    Implements the ``RemoteConnection`` protocol. All I/O goes through
    ``docker exec`` so the connection survives workspace mutations inside
    the container.
    """

    # ------------------------------------------------------------------
    # Construction
    # ------------------------------------------------------------------

    def __init__(self, *, container_id: str, socket_url: str | None = None) -> None:
        """Initialize with a container ID and optional Docker socket URL.

        Args:
            container_id: ID (or name) of the running container.
            socket_url: Docker daemon socket URL, ``None`` for the default.
        """
        self._container_id = container_id
        self._socket_url = socket_url
        self._docker: aiodocker.Docker | None = None

    @property
    def container_id(self) -> str:
        """Return the Docker container ID."""
        return self._container_id

    def _ensure_client(self) -> aiodocker.Docker:
        """Return the cached aiodocker client, creating one on first call.

        Returns:
            An ``aiodocker.Docker`` client instance.

        Raises:
            ConnectionError: If the client cannot be created.
        """
        if self._docker is not None:
            return self._docker

        try:
            import aiodocker as _aiodocker  # noqa: PLC0415 — optional dep

            self._docker = _aiodocker.Docker(url=self._socket_url)
        except Exception as exc:
            raise ConnectionError(
                f"Failed to create Docker client (url={self._socket_url}): {exc}"
            ) from exc

        return self._docker

    # ------------------------------------------------------------------
    # Factory class-methods
    # ------------------------------------------------------------------

    @classmethod
    async def create_and_start(
        cls,
        config: DockerConfig,
        *,
        name: str,
        labels: dict[str, str] | None = None,
        cpu_limit: float | None = None,
        memory_limit_bytes: int | None = None,
    ) -> DockerConnection:
        """Create a new container from *config*, start it, and return a connection.

        Args:
            config: Docker container configuration.
            name: Container name.
            labels: Optional labels to attach to the container.
            cpu_limit: CPU limit in cores (e.g. ``1.5`` = 1.5 cores).
            memory_limit_bytes: Hard memory limit in bytes.

        Returns:
            A ``DockerConnection`` connected to the newly started container.

        Raises:
            ConnectionError: If the Docker client cannot be created.
        """
        try:
            import aiodocker as _aiodocker  # noqa: PLC0415 — optional dep
        except ModuleNotFoundError:
            raise ConnectionError(
                "aiodocker is required for Docker execution. "
                "Install it with: uv sync --extra docker"
            ) from None

        try:
            docker = _aiodocker.Docker(url=config.socket_url)
        except Exception as exc:
            raise ConnectionError(
                f"Failed to create Docker client (url={config.socket_url}): {exc}"
            ) from exc

        # Best-effort image pull — the image may already exist locally.
        try:
            await docker.images.pull(config.image)
        except Exception:
            logger.warning(
                "Failed to pull image %s; proceeding in case it exists locally",
                config.image,
            )

        # Build host-config section.
        host_config: dict[str, JSONObject | list[str] | int | str] = {
            "Binds": list(config.extra_volumes),
        }
        if cpu_limit is not None:
            host_config["NanoCpus"] = int(cpu_limit * 1e9)
        if memory_limit_bytes is not None:
            host_config["Memory"] = memory_limit_bytes
        if config.network is not None:
            host_config["NetworkMode"] = config.network

        # Build container-config dict.
        container_config: JSONObject = {
            "Image": config.image,
            "Cmd": ["sleep", "infinity"],
            "Labels": labels or {},
            "Tty": False,
            "HostConfig": host_config,
        }
        if config.extra_env:
            container_config["Env"] = [f"{k}={v}" for k, v in config.extra_env.items()]

        container = None
        try:
            container = await docker.containers.create_or_replace(name, container_config)
            await container.start()
        except BaseException:
            # Clean up partially-created container and docker client so
            # callers don't need to know about partial state.
            if container is not None:
                with contextlib.suppress(Exception):
                    await container.delete(force=True)
            with contextlib.suppress(Exception):
                await docker.close()
            raise

        conn = cls(container_id=container.id, socket_url=config.socket_url)
        # Re-use the client we already created instead of making a new one.
        conn._docker = docker
        logger.info("Started Docker container %s (image=%s)", name, config.image)
        return conn

    @classmethod
    def from_existing(cls, container_id: str, socket_url: str | None = None) -> DockerConnection:
        """Wrap an already-running container without creating a client yet.

        Args:
            container_id: ID (or name) of the running container.
            socket_url: Docker daemon socket URL, ``None`` for the default.

        Returns:
            A ``DockerConnection`` with lazy client initialization.
        """
        return cls(container_id=container_id, socket_url=socket_url)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    async def _exec_and_collect(
        self,
        command: str,
        *,
        tty: bool = False,
        timeout_secs: int | None = None,
    ) -> tuple[int, str, str]:
        """Run a command inside the container and collect output.

        Uses ``Stream.read_out()`` to read ``Message`` objects from the
        exec stream.  When *tty* is ``False`` each message carries a
        *stream* field (1 = stdout, 2 = stderr) so we can separate them.
        When *tty* is ``True`` the Docker daemon merges both into stream 1.

        Args:
            command: Shell command string to execute via ``/bin/bash -c``.
            tty: Whether to allocate a pseudo-TTY for the exec session.
            timeout_secs: Wall-clock timeout in seconds, ``None`` for unlimited.

        Returns:
            Tuple of ``(exit_code, stdout, stderr)``.
        """
        docker = self._ensure_client()
        container = docker.containers.container(self._container_id)

        exec_instance = await container.exec(
            cmd=["/bin/bash", "-c", command],
            stdout=True,
            stderr=True,
            tty=tty,
        )

        stream = exec_instance.start(detach=False)

        async def _collect() -> tuple[bytes, bytes]:
            stdout_chunks: list[bytes] = []
            stderr_chunks: list[bytes] = []
            while True:
                msg = await stream.read_out()
                if msg is None:
                    break
                if msg.stream == 2:
                    stderr_chunks.append(msg.data)
                else:
                    # stream 1 (stdout) or any other value → stdout
                    stdout_chunks.append(msg.data)
            return b"".join(stdout_chunks), b"".join(stderr_chunks)

        try:
            if timeout_secs is not None:
                stdout_bytes, stderr_bytes = await asyncio.wait_for(
                    _collect(), timeout=float(timeout_secs)
                )
            else:
                stdout_bytes, stderr_bytes = await _collect()
        except TimeoutError:
            with contextlib.suppress(Exception):
                await stream.close()
            return -1, "", "Command timed out"

        inspect = await exec_instance.inspect()
        exit_code: int = inspect["ExitCode"]

        return (
            exit_code,
            stdout_bytes.decode(errors="replace"),
            stderr_bytes.decode(errors="replace"),
        )

    # ------------------------------------------------------------------
    # RemoteConnection protocol methods
    # ------------------------------------------------------------------

    async def run(
        self,
        command: str,
        *,
        timeout_secs: int | None = None,
        stdin_data: str | None = None,
        request_pty: bool = False,
    ) -> RemoteResult:
        """Execute a command inside the container.

        Args:
            command: Shell command to execute.
            timeout_secs: Wall-clock timeout in seconds.
            stdin_data: Optional string to pipe into the command's stdin.
            request_pty: Allocate a pseudo-TTY for the exec session.

        Returns:
            ``RemoteResult`` with exit code, stdout, stderr, and timeout flag.
        """
        if stdin_data is not None:
            encoded = base64.b64encode(stdin_data.encode()).decode()
            command = f"echo '{encoded}' | base64 -d | {command}"

        exit_code, stdout, stderr = await self._exec_and_collect(
            command,
            tty=request_pty,
            timeout_secs=timeout_secs,
        )

        return RemoteResult(
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            timed_out=(exit_code == -1 and stderr == "Command timed out"),
        )

    async def upload_content(self, content: str, remote_path: str) -> None:
        """Upload string content to a file inside the container.

        Creates parent directories as needed.

        Args:
            content: File content to write.
            remote_path: Absolute path inside the container.

        Raises:
            OSError: If the write command fails.
        """
        encoded = base64.b64encode(content.encode()).decode()
        dir_path = shlex.quote(remote_path.rsplit("/", 1)[0]) if "/" in remote_path else "."
        quoted_path = shlex.quote(remote_path)
        cmd = (
            f"mkdir -p {dir_path} && printf '%s' {shlex.quote(encoded)} | base64 -d > {quoted_path}"
        )
        result = await self.run(cmd, timeout_secs=30)
        if result.exit_code != 0:
            msg = result.stderr or result.stdout
            raise OSError(f"upload_content to {remote_path} failed: {msg}")

    async def download_content(self, remote_path: str) -> str | None:
        """Download content from a file inside the container.

        Args:
            remote_path: Absolute path inside the container.

        Returns:
            File content as a string, or ``None`` if the file does not exist.
        """
        result = await self.run(
            f"cat {shlex.quote(remote_path)} 2>/dev/null",
            timeout_secs=30,
        )
        if result.exit_code != 0:
            return None
        return result.stdout

    async def check_connection(self) -> bool:
        """Test connectivity by running a simple echo inside the container.

        Returns:
            ``True`` if the container is reachable and responsive.
        """
        try:
            result = await self.run("echo tanren-ok", timeout_secs=10)
        except Exception:
            logger.debug(
                "check_connection failed for container %s",
                self._container_id[:12],
                exc_info=True,
            )
            return False
        return result.exit_code == 0 and "tanren-ok" in result.stdout

    def get_host_identifier(self) -> str:
        """Return a human-readable identifier for this container.

        Returns:
            String in ``docker://<short-id>`` format.
        """
        return f"docker://{self._container_id[:12]}"

    # ------------------------------------------------------------------
    # Lifecycle helpers (not part of RemoteConnection)
    # ------------------------------------------------------------------

    async def stop_container(self, *, stop_timeout: int = 10) -> None:
        """Stop the container gracefully.

        Args:
            stop_timeout: Seconds to wait before force-killing.
        """
        docker = self._ensure_client()
        container = docker.containers.container(self._container_id)
        await container.stop(t=stop_timeout)

    async def remove_container(self, *, force: bool = True) -> None:
        """Remove the container.

        Args:
            force: Force-remove even if the container is running.
        """
        docker = self._ensure_client()
        container = docker.containers.container(self._container_id)
        await container.delete(force=force)

    async def close(self) -> None:
        """Close the underlying aiodocker client."""
        if self._docker is not None:
            with contextlib.suppress(Exception):
                await self._docker.close()
            self._docker = None
