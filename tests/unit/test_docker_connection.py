"""Tests for Docker container connection adapter."""

from __future__ import annotations

import base64
import logging
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from tanren_core.adapters.docker_connection import DockerConfig, DockerConnection
from tanren_core.adapters.remote_types import RemoteResult

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_conn(
    container_id: str = "abc123def456",
    socket_url: str | None = None,
) -> DockerConnection:
    return DockerConnection(container_id=container_id, socket_url=socket_url)


def _mock_exec_instance(
    output: bytes = b"",
    exit_code: int = 0,
    *,
    stderr_output: bytes = b"",
) -> MagicMock:
    """Return a mock aiodocker exec instance with a Stream-like start().

    ``start(detach=False)`` is called *without* await in the implementation
    and returns a Stream object whose ``read_out()`` yields ``Message``
    namedtuples.  ``inspect()`` *is* awaited.
    """
    from collections import namedtuple

    Message = namedtuple("Message", ["stream", "data"])
    exec_inst = MagicMock()

    # Build the message sequence: stdout messages then stderr messages.
    messages: list[Message | None] = []
    if output:
        messages.append(Message(stream=1, data=output))
    if stderr_output:
        messages.append(Message(stream=2, data=stderr_output))
    messages.append(None)  # sentinel for end-of-stream

    def _make_stream(detach: bool = False) -> MagicMock:
        it = iter(messages)
        stream = AsyncMock()
        stream.read_out = AsyncMock(side_effect=lambda: next(it))
        stream.close = AsyncMock()
        return stream

    exec_inst.start = MagicMock(side_effect=_make_stream)
    exec_inst.inspect = AsyncMock(return_value={"ExitCode": exit_code})
    return exec_inst


def _patch_ensure_client(conn: DockerConnection, exec_inst: MagicMock) -> MagicMock:
    """Wire up a mock Docker client -> container -> exec chain on *conn*."""
    mock_docker = MagicMock()
    mock_container = AsyncMock()
    mock_docker.containers.container.return_value = mock_container
    mock_container.exec.return_value = exec_inst
    conn._docker = mock_docker
    return mock_docker


# ---------------------------------------------------------------------------
# TestDockerConnectionRun
# ---------------------------------------------------------------------------


class TestDockerConnectionRun:
    async def test_run_simple_command(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"hello\n", exit_code=0)
        _patch_ensure_client(conn, exec_inst)

        result = await conn.run("echo hello")

        assert isinstance(result, RemoteResult)
        assert result.exit_code == 0
        assert "hello" in result.stdout
        assert result.timed_out is False

    async def test_run_exit_code_propagation(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"error msg\n", exit_code=127)
        _patch_ensure_client(conn, exec_inst)

        result = await conn.run("nonexistent-cmd")

        assert result.exit_code == 127
        assert result.timed_out is False

    async def test_run_timeout(self):
        conn = _make_conn()

        exec_inst = MagicMock()

        async def _hanging_stream():
            import asyncio

            await asyncio.sleep(999)
            yield b""  # pragma: no cover

        exec_inst.start = MagicMock(side_effect=lambda detach=False: _hanging_stream())
        exec_inst.inspect = AsyncMock(return_value={"ExitCode": 0})

        _patch_ensure_client(conn, exec_inst)

        result = await conn.run("sleep 999", timeout_secs=0)

        assert result.timed_out is True
        assert result.exit_code == -1
        assert "timed out" in result.stderr.lower()

    async def test_run_with_stdin_data(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"piped\n", exit_code=0)
        mock_docker = _patch_ensure_client(conn, exec_inst)

        await conn.run("cat", stdin_data="hello world")

        # Verify the command was wrapped with base64 piping
        mock_container = mock_docker.containers.container.return_value
        call_args = mock_container.exec.call_args
        cmd_list = call_args.kwargs.get("cmd") or call_args[1].get("cmd") or call_args[0][0]
        actual_command = cmd_list[2]  # /bin/bash -c <command>
        encoded = base64.b64encode(b"hello world").decode()
        assert f"echo '{encoded}' | base64 -d | cat" == actual_command

    async def test_run_with_request_pty(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"tty-out\n", exit_code=0)
        mock_docker = _patch_ensure_client(conn, exec_inst)

        await conn.run("cmd", request_pty=True)

        mock_container = mock_docker.containers.container.return_value
        call_kwargs = mock_container.exec.call_args.kwargs
        assert call_kwargs["tty"] is True


# ---------------------------------------------------------------------------
# TestDockerConnectionUploadContent
# ---------------------------------------------------------------------------


class TestDockerConnectionUploadContent:
    async def test_upload_creates_parent_dir(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"", exit_code=0)
        mock_docker = _patch_ensure_client(conn, exec_inst)

        await conn.upload_content("file body", "/tmp/sub/test.txt")

        mock_container = mock_docker.containers.container.return_value
        call_args = mock_container.exec.call_args
        cmd_list = call_args.kwargs.get("cmd") or call_args[0][0]
        actual_command = cmd_list[2]

        assert "mkdir -p" in actual_command
        assert "/tmp/sub" in actual_command
        assert "base64 -d" in actual_command

    async def test_upload_raises_on_failure(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"permission denied\n", exit_code=1)
        _patch_ensure_client(conn, exec_inst)

        with pytest.raises(OSError, match=r"upload_content to /root/secret\.txt failed"):
            await conn.upload_content("data", "/root/secret.txt")


# ---------------------------------------------------------------------------
# TestDockerConnectionDownloadContent
# ---------------------------------------------------------------------------


class TestDockerConnectionDownloadContent:
    async def test_download_existing_file(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"file content here", exit_code=0)
        _patch_ensure_client(conn, exec_inst)

        content = await conn.download_content("/tmp/test.txt")

        assert content == "file content here"

    async def test_download_missing_file(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"", exit_code=1)
        _patch_ensure_client(conn, exec_inst)

        content = await conn.download_content("/tmp/nonexistent.txt")

        assert content is None


# ---------------------------------------------------------------------------
# TestDockerConnectionCheckConnection
# ---------------------------------------------------------------------------


class TestDockerConnectionCheckConnection:
    async def test_check_returns_true_when_running(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"tanren-ok\n", exit_code=0)
        _patch_ensure_client(conn, exec_inst)

        assert await conn.check_connection() is True

    async def test_check_returns_false_on_error(self):
        conn = _make_conn()
        # Make _ensure_client raise to simulate unreachable container
        conn._docker = None
        with patch.object(
            DockerConnection,
            "_ensure_client",
            side_effect=ConnectionError("cannot connect"),
        ):
            assert await conn.check_connection() is False

    async def test_check_returns_false_on_nonzero_exit(self):
        conn = _make_conn()
        exec_inst = _mock_exec_instance(output=b"", exit_code=1)
        _patch_ensure_client(conn, exec_inst)

        assert await conn.check_connection() is False

    async def test_check_connection_logs_failure_at_debug(self, caplog):
        conn = _make_conn()
        conn._docker = None
        with (
            patch.object(
                DockerConnection,
                "_ensure_client",
                side_effect=Exception("container gone"),
            ),
            caplog.at_level(logging.DEBUG, logger="tanren_core.adapters.docker_connection"),
        ):
            result = await conn.check_connection()

        assert result is False
        assert "check_connection failed" in caplog.text


# ---------------------------------------------------------------------------
# TestDockerConnectionClose
# ---------------------------------------------------------------------------


class TestDockerConnectionClose:
    async def test_close_closes_client(self):
        conn = _make_conn()
        mock_docker = AsyncMock()
        conn._docker = mock_docker

        await conn.close()

        mock_docker.close.assert_awaited_once()
        assert conn._docker is None

    async def test_close_idempotent(self):
        conn = _make_conn()
        mock_docker = AsyncMock()
        conn._docker = mock_docker

        await conn.close()
        await conn.close()

        # close() on the client is only called once because _docker is set to None
        mock_docker.close.assert_awaited_once()
        assert conn._docker is None

    async def test_close_suppresses_exceptions(self):
        conn = _make_conn()
        mock_docker = AsyncMock()
        mock_docker.close.side_effect = Exception("socket error")
        conn._docker = mock_docker

        # Should not raise
        await conn.close()
        assert conn._docker is None


# ---------------------------------------------------------------------------
# TestDockerConnectionFromExisting
# ---------------------------------------------------------------------------


class TestDockerConnectionFromExisting:
    async def test_creates_instance_without_client(self):
        conn = DockerConnection.from_existing("deadbeef1234")

        assert conn._container_id == "deadbeef1234"
        assert conn._docker is None

    async def test_lazy_client_created_on_first_run(self):
        conn = DockerConnection.from_existing("deadbeef1234")
        assert conn._docker is None

        # Patch the aiodocker import inside _ensure_client
        mock_aiodocker = MagicMock()
        mock_docker_client = MagicMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        # Set up the exec chain on the lazily-created client
        mock_container = AsyncMock()
        exec_inst = _mock_exec_instance(output=b"ok\n", exit_code=0)
        mock_docker_client.containers.container.return_value = mock_container
        mock_container.exec.return_value = exec_inst

        with patch.dict("sys.modules", {"aiodocker": mock_aiodocker}):
            result = await conn.run("echo ok")

        assert conn._docker is mock_docker_client
        assert result.exit_code == 0

    async def test_from_existing_with_socket_url(self):
        conn = DockerConnection.from_existing("deadbeef1234", socket_url="unix:///custom.sock")

        assert conn._container_id == "deadbeef1234"
        assert conn._socket_url == "unix:///custom.sock"
        assert conn._docker is None


# ---------------------------------------------------------------------------
# TestDockerConnectionCreateAndStart
# ---------------------------------------------------------------------------


class TestDockerConnectionCreateAndStart:
    async def test_pulls_image_and_creates_container(self):
        mock_aiodocker = MagicMock()
        mock_docker_client = AsyncMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        mock_container = AsyncMock()
        mock_container.id = "new-container-123"
        mock_docker_client.containers.create_or_replace.return_value = mock_container
        mock_docker_client.images.pull = AsyncMock()

        config = DockerConfig(image="python:3.14")

        with patch.dict("sys.modules", {"aiodocker": mock_aiodocker}):
            conn = await DockerConnection.create_and_start(config, name="test-vm")

        mock_docker_client.images.pull.assert_awaited_once_with("python:3.14")
        mock_docker_client.containers.create_or_replace.assert_awaited_once()
        mock_container.start.assert_awaited_once()
        assert conn._container_id == "new-container-123"
        assert conn._docker is mock_docker_client

    async def test_cpu_and_memory_limits_in_config(self):
        mock_aiodocker = MagicMock()
        mock_docker_client = AsyncMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        mock_container = AsyncMock()
        mock_container.id = "limited-container"
        mock_docker_client.containers.create_or_replace.return_value = mock_container
        mock_docker_client.images.pull = AsyncMock()

        config = DockerConfig(image="ubuntu:24.04")

        with patch.dict("sys.modules", {"aiodocker": mock_aiodocker}):
            await DockerConnection.create_and_start(
                config,
                name="limited-vm",
                cpu_limit=2.0,
                memory_limit_bytes=4_000_000_000,
            )

        call_args = mock_docker_client.containers.create_or_replace.call_args
        container_config = call_args[0][1]  # positional: name, config
        host_config = container_config["HostConfig"]

        assert host_config["NanoCpus"] == int(2.0 * 1e9)
        assert host_config["Memory"] == 4_000_000_000

    async def test_network_in_config(self):
        mock_aiodocker = MagicMock()
        mock_docker_client = AsyncMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        mock_container = AsyncMock()
        mock_container.id = "networked-container"
        mock_docker_client.containers.create_or_replace.return_value = mock_container
        mock_docker_client.images.pull = AsyncMock()

        config = DockerConfig(image="ubuntu:24.04", network="my-network")

        with patch.dict("sys.modules", {"aiodocker": mock_aiodocker}):
            await DockerConnection.create_and_start(config, name="net-vm")

        call_args = mock_docker_client.containers.create_or_replace.call_args
        container_config = call_args[0][1]
        host_config = container_config["HostConfig"]

        assert host_config["NetworkMode"] == "my-network"

    async def test_labels_applied(self):
        mock_aiodocker = MagicMock()
        mock_docker_client = AsyncMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        mock_container = AsyncMock()
        mock_container.id = "labeled-container"
        mock_docker_client.containers.create_or_replace.return_value = mock_container
        mock_docker_client.images.pull = AsyncMock()

        config = DockerConfig(image="ubuntu:24.04")
        labels = {"app": "tanren", "env": "test"}

        with patch.dict("sys.modules", {"aiodocker": mock_aiodocker}):
            await DockerConnection.create_and_start(config, name="label-vm", labels=labels)

        call_args = mock_docker_client.containers.create_or_replace.call_args
        container_config = call_args[0][1]

        assert container_config["Labels"] == {"app": "tanren", "env": "test"}

    async def test_pull_failure_logged_not_raised(self, caplog):
        mock_aiodocker = MagicMock()
        mock_docker_client = AsyncMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        mock_container = AsyncMock()
        mock_container.id = "fallback-container"
        mock_docker_client.containers.create_or_replace.return_value = mock_container
        mock_docker_client.images.pull = AsyncMock(side_effect=Exception("network error"))

        config = DockerConfig(image="my-local-image:latest")

        with (
            patch.dict("sys.modules", {"aiodocker": mock_aiodocker}),
            caplog.at_level(logging.WARNING, logger="tanren_core.adapters.docker_connection"),
        ):
            conn = await DockerConnection.create_and_start(config, name="pull-fail-vm")

        # Container should still be created and started despite pull failure
        mock_docker_client.containers.create_or_replace.assert_awaited_once()
        mock_container.start.assert_awaited_once()
        assert conn._container_id == "fallback-container"
        assert "Failed to pull image" in caplog.text

    async def test_extra_env_in_config(self):
        mock_aiodocker = MagicMock()
        mock_docker_client = AsyncMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        mock_container = AsyncMock()
        mock_container.id = "env-container"
        mock_docker_client.containers.create_or_replace.return_value = mock_container
        mock_docker_client.images.pull = AsyncMock()

        config = DockerConfig(image="ubuntu:24.04", extra_env={"FOO": "bar", "BAZ": "qux"})

        with patch.dict("sys.modules", {"aiodocker": mock_aiodocker}):
            await DockerConnection.create_and_start(config, name="env-vm")

        call_args = mock_docker_client.containers.create_or_replace.call_args
        container_config = call_args[0][1]

        assert "Env" in container_config
        assert "FOO=bar" in container_config["Env"]
        assert "BAZ=qux" in container_config["Env"]

    async def test_extra_volumes_in_config(self):
        mock_aiodocker = MagicMock()
        mock_docker_client = AsyncMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        mock_container = AsyncMock()
        mock_container.id = "vol-container"
        mock_docker_client.containers.create_or_replace.return_value = mock_container
        mock_docker_client.images.pull = AsyncMock()

        config = DockerConfig(
            image="ubuntu:24.04",
            extra_volumes=("/host/path:/container/path", "/data:/data:ro"),
        )

        with patch.dict("sys.modules", {"aiodocker": mock_aiodocker}):
            await DockerConnection.create_and_start(config, name="vol-vm")

        call_args = mock_docker_client.containers.create_or_replace.call_args
        container_config = call_args[0][1]
        host_config = container_config["HostConfig"]

        assert host_config["Binds"] == ["/host/path:/container/path", "/data:/data:ro"]

    async def test_docker_client_creation_failure_raises_connection_error(self):
        mock_aiodocker = MagicMock()
        mock_aiodocker.Docker.side_effect = Exception("socket not found")

        config = DockerConfig(image="ubuntu:24.04")

        with (
            patch.dict("sys.modules", {"aiodocker": mock_aiodocker}),
            pytest.raises(ConnectionError, match="Failed to create Docker client"),
        ):
            await DockerConnection.create_and_start(config, name="fail-vm")


# ---------------------------------------------------------------------------
# TestDockerConnectionGetHostIdentifier
# ---------------------------------------------------------------------------


class TestDockerConnectionGetHostIdentifier:
    async def test_returns_docker_prefix(self):
        conn = _make_conn(container_id="abc123def456789xyz")

        assert conn.get_host_identifier() == "docker://abc123def456"

    async def test_short_id_unchanged(self):
        conn = _make_conn(container_id="short")

        assert conn.get_host_identifier() == "docker://short"


# ---------------------------------------------------------------------------
# TestDockerConnectionStopContainer
# ---------------------------------------------------------------------------


class TestDockerConnectionStopContainer:
    async def test_stop_calls_container_stop(self):
        conn = _make_conn()
        mock_docker = MagicMock()
        mock_container = AsyncMock()
        mock_docker.containers.container.return_value = mock_container
        conn._docker = mock_docker

        await conn.stop_container()

        mock_container.stop.assert_awaited_once_with(t=10)

    async def test_stop_with_custom_timeout(self):
        conn = _make_conn()
        mock_docker = MagicMock()
        mock_container = AsyncMock()
        mock_docker.containers.container.return_value = mock_container
        conn._docker = mock_docker

        await conn.stop_container(stop_timeout=30)

        mock_container.stop.assert_awaited_once_with(t=30)


# ---------------------------------------------------------------------------
# TestDockerConnectionRemoveContainer
# ---------------------------------------------------------------------------


class TestDockerConnectionRemoveContainer:
    async def test_remove_calls_container_delete(self):
        conn = _make_conn()
        mock_docker = MagicMock()
        mock_container = AsyncMock()
        mock_docker.containers.container.return_value = mock_container
        conn._docker = mock_docker

        await conn.remove_container()

        mock_container.delete.assert_awaited_once_with(force=True)

    async def test_remove_without_force(self):
        conn = _make_conn()
        mock_docker = MagicMock()
        mock_container = AsyncMock()
        mock_docker.containers.container.return_value = mock_container
        conn._docker = mock_docker

        await conn.remove_container(force=False)

        mock_container.delete.assert_awaited_once_with(force=False)


# ---------------------------------------------------------------------------
# TestDockerConnectionEnsureClient
# ---------------------------------------------------------------------------


class TestDockerConnectionEnsureClient:
    async def test_ensure_client_raises_connection_error(self):
        conn = _make_conn()

        mock_aiodocker = MagicMock()
        mock_aiodocker.Docker.side_effect = Exception("no socket")

        with (
            patch.dict("sys.modules", {"aiodocker": mock_aiodocker}),
            pytest.raises(ConnectionError, match="Failed to create Docker client"),
        ):
            conn._ensure_client()

    async def test_ensure_client_reuses_existing(self):
        conn = _make_conn()
        mock_docker = MagicMock()
        conn._docker = mock_docker

        result = conn._ensure_client()

        assert result is mock_docker

    async def test_ensure_client_passes_socket_url(self):
        conn = _make_conn(socket_url="tcp://remote:2375")

        mock_aiodocker = MagicMock()
        mock_docker_client = MagicMock()
        mock_aiodocker.Docker.return_value = mock_docker_client

        with patch.dict("sys.modules", {"aiodocker": mock_aiodocker}):
            result = conn._ensure_client()

        mock_aiodocker.Docker.assert_called_once_with(url="tcp://remote:2375")
        assert result is mock_docker_client


# ---------------------------------------------------------------------------
# TestDockerConfig
# ---------------------------------------------------------------------------


class TestDockerConfig:
    def test_defaults(self):
        cfg = DockerConfig()
        assert cfg.image == "ubuntu:24.04"
        assert cfg.socket_url is None
        assert cfg.network is None
        assert cfg.extra_volumes == ()
        assert cfg.extra_env == {}
        assert cfg.api_version == "v1.45"

    def test_frozen(self):
        from pydantic import ValidationError

        cfg = DockerConfig()
        with pytest.raises(ValidationError, match="frozen"):
            cfg.image = "other"
