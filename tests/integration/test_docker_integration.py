"""Integration tests for DockerConnection — requires a running Docker daemon.

Run with: uv run pytest tests/integration/test_docker_integration.py -v -m docker --timeout=120
"""

from __future__ import annotations

import contextlib

import pytest

from tanren_core.adapters.docker_connection import DockerConfig, DockerConnection

pytestmark = pytest.mark.docker


@pytest.fixture
async def docker_conn():
    """Create a real container and yield a DockerConnection, then cleanup."""
    config = DockerConfig(image="ubuntu:24.04")
    conn = await DockerConnection.create_and_start(
        config,
        name="tanren-test-integration",
        labels={"tanren.test": "true"},
    )
    try:
        yield conn
    finally:
        with contextlib.suppress(Exception):
            await conn.stop_container()
        with contextlib.suppress(Exception):
            await conn.remove_container()
        await conn.close()


async def test_run_echo(docker_conn: DockerConnection):
    """Run a simple echo command and verify output."""
    result = await docker_conn.run("echo hello", timeout_secs=10)
    assert result.exit_code == 0
    assert "hello" in result.stdout


async def test_run_exit_code(docker_conn: DockerConnection):
    """Run a failing command and verify nonzero exit code."""
    result = await docker_conn.run("false", timeout_secs=10)
    assert result.exit_code == 1


async def test_upload_download_roundtrip(docker_conn: DockerConnection):
    """Upload content and download it back, verifying a perfect roundtrip."""
    content = "test content"
    remote_path = "/tmp/tanren-test-roundtrip.txt"

    await docker_conn.upload_content(content, remote_path)
    downloaded = await docker_conn.download_content(remote_path)

    assert downloaded == content


async def test_upload_special_characters(docker_conn: DockerConnection):
    """Upload content with single quotes, newlines, and dollar signs."""
    content = "it's a $HOME\nwith newlines\nand 'quotes' $VAR\n"
    remote_path = "/tmp/tanren-test-special.txt"

    await docker_conn.upload_content(content, remote_path)
    downloaded = await docker_conn.download_content(remote_path)

    assert downloaded == content


async def test_download_missing_file(docker_conn: DockerConnection):
    """download_content returns None for a nonexistent path."""
    result = await docker_conn.download_content("/tmp/tanren-nonexistent-file.txt")
    assert result is None


async def test_check_connection(docker_conn: DockerConnection):
    """Verify check_connection returns True for a running container."""
    assert await docker_conn.check_connection() is True


def test_get_host_identifier(docker_conn: DockerConnection):
    """Verify host identifier starts with docker:// prefix."""
    identifier = docker_conn.get_host_identifier()
    assert identifier.startswith("docker://")


async def test_run_timeout(docker_conn: DockerConnection):
    """Run a long-sleeping command with a short timeout and verify it times out."""
    result = await docker_conn.run("sleep 60", timeout_secs=2)
    assert result.timed_out is True
