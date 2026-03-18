"""Integration tests for Hetzner VM provisioning.

These tests provision real Hetzner VMs. Secrets are loaded from
~/.config/tanren/secrets.env automatically. Skip when HETZNER_API_TOKEN
is not available.

Run with:
    uv run pytest tests/integration/test_hetzner_provisioning.py -v --timeout=600
"""

from __future__ import annotations

import asyncio
import os
import time

import pytest

from tanren_core.adapters.hetzner_vm import HetznerProvisionerSettings, HetznerVMProvisioner
from tanren_core.adapters.remote_types import VMProvider, VMRequirements
from tanren_core.adapters.ssh import SSHConfig, SSHConnection
from tanren_core.schemas import Cli
from tanren_core.secrets import SecretLoader

pytestmark = pytest.mark.hetzner


def _load_token() -> str | None:
    """Load HETZNER_API_TOKEN from the developer secrets file."""
    loader = SecretLoader(required_clis=frozenset({Cli.CLAUDE}))
    loader.autoload_into_env(override=False)
    secrets = loader.load_developer()
    return secrets.get("HETZNER_API_TOKEN")


_TOKEN = _load_token()

_skip_reason = "HETZNER_API_TOKEN not found in secrets.env"


@pytest.fixture()
def provisioner():
    """Create a HetznerVMProvisioner using developer secrets and real config values."""
    if not _TOKEN:
        raise pytest.skip.Exception(_skip_reason)

    os.environ.setdefault("HETZNER_API_TOKEN", _TOKEN)

    settings = HetznerProvisionerSettings(
        token_env="HETZNER_API_TOKEN",
        default_server_type="cpx41",
        location="hil",
        image="ubuntu-24.04",
        ssh_key_name="aegis-tanren",
        name_prefix="tanren-test",
        labels={"env": "test"},
    )
    return HetznerVMProvisioner(settings)


@pytest.fixture()
def requirements():
    return VMRequirements(profile="test", cpu=2, memory_gb=4, gpu=False)


class TestHetznerAcquireAndSSH:
    @pytest.mark.timeout(600)
    async def test_acquire_wait_ssh_release(self, provisioner, requirements):
        """Acquire VM -> wait for SSH -> run command -> release."""
        vm = await provisioner.acquire(requirements)
        try:
            assert vm.provider == VMProvider.HETZNER
            assert vm.host

            conn = SSHConnection(
                SSHConfig(
                    host=vm.host,
                    user="root",
                    key_path="~/.ssh/hetzner_tanren",
                    connect_timeout=10,
                )
            )
            try:
                deadline = time.monotonic() + 300
                while time.monotonic() < deadline:
                    if await conn.check_connection():
                        break
                    await asyncio.sleep(3)
                else:
                    raise AssertionError(f"SSH not ready within 300s on {vm.host}")

                result = await conn.run("echo tanren-ok")
                assert result.exit_code == 0
                assert "tanren-ok" in result.stdout
            finally:
                await conn.close()
        finally:
            await provisioner.release(vm)


class TestHetznerProvisionLifecycle:
    @pytest.mark.timeout(600)
    async def test_provision_and_teardown(self, provisioner, requirements):
        """Full acquire -> release lifecycle without SSH."""
        vm = await provisioner.acquire(requirements)
        try:
            assert vm.vm_id
            assert vm.host
            assert vm.provider == VMProvider.HETZNER

            active = await provisioner.list_active()
            vm_ids = [h.vm_id for h in active]
            assert vm.vm_id in vm_ids
        finally:
            await provisioner.release(vm)
