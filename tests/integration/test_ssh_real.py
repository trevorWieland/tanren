"""Real SSH integration tests — requires --ssh-host and --ssh-key."""

from __future__ import annotations

import pytest

from tanren_core.adapters.ssh import SSHConfig, SSHConnection

pytestmark = pytest.mark.ssh


@pytest.fixture
def ssh_config(request):
    """Build SSHConfig from CLI options."""
    host = request.config.getoption("--ssh-host")
    key = request.config.getoption("--ssh-key")
    user = request.config.getoption("--ssh-user")
    if not host or not key:
        raise ValueError("--ssh-host and --ssh-key are required for SSH tests")
    return SSHConfig(host=host, key_path=key, user=user)


async def test_real_ssh_connect_and_run(ssh_config):
    """Connect and run a simple command."""
    conn = SSHConnection(ssh_config)
    try:
        result = await conn.run("echo hello", timeout_secs=10)
        assert result.exit_code == 0
        assert "hello" in result.stdout
    finally:
        await conn.close()


async def test_real_ssh_upload_download(ssh_config):
    """Upload content and download it back."""
    conn = SSHConnection(ssh_config)
    try:
        content = "test-content-12345"
        await conn.upload_content(content, "/tmp/tanren-test-upload.txt")
        downloaded = await conn.download_content("/tmp/tanren-test-upload.txt")
        assert downloaded == content
        await conn.run("rm /tmp/tanren-test-upload.txt", timeout_secs=10)
    finally:
        await conn.close()


async def test_real_ssh_check_connection(ssh_config):
    """Test connectivity check."""
    conn = SSHConnection(ssh_config)
    try:
        assert await conn.check_connection() is True
    finally:
        await conn.close()


async def test_real_ssh_download_missing_file(ssh_config):
    """download_content returns None for missing files."""
    conn = SSHConnection(ssh_config)
    try:
        result = await conn.download_content("/tmp/tanren-nonexistent-file.txt")
        assert result is None
    finally:
        await conn.close()
