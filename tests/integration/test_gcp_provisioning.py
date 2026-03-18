"""Integration tests for GCP Compute Engine VM provisioning.

These tests provision real GCP VMs. Skip when GOOGLE_CLOUD_PROJECT
env var is not available.

Run with:
    uv run pytest tests/integration/test_gcp_provisioning.py -v --timeout=600
"""

from __future__ import annotations

import asyncio
import os
import time

import pytest

from tanren_core.adapters.gcp_vm import GCPProvisionerSettings, GCPVMProvisioner
from tanren_core.adapters.remote_types import VMProvider, VMRequirements
from tanren_core.adapters.ssh import SSHConfig, SSHConnection

pytestmark = pytest.mark.gcp

_PROJECT = os.environ.get("GOOGLE_CLOUD_PROJECT")
_skip_reason = "GOOGLE_CLOUD_PROJECT not set"


@pytest.fixture()
def provisioner():
    """Create a GCPVMProvisioner using real GCP credentials."""
    if not _PROJECT:
        raise pytest.skip.Exception(_skip_reason)
    if not os.environ.get("GCP_SSH_PUBLIC_KEY"):
        raise pytest.skip.Exception("GCP_SSH_PUBLIC_KEY not set")

    settings = GCPProvisionerSettings(
        project_id=_PROJECT,
        zone="us-central1-a",
        default_machine_type="e2-standard-4",
        image_family="ubuntu-2404-lts-amd64",
        name_prefix="tanren-test",
        labels={"env": "test"},
    )
    return GCPVMProvisioner(settings)


@pytest.fixture()
def requirements():
    return VMRequirements(profile="test", cpu=2, memory_gb=4, gpu=False)


class TestGCPProvisionLifecycle:
    @pytest.mark.timeout(600)
    async def test_provision_and_teardown(self, provisioner, requirements):
        """Full acquire -> list_active -> release lifecycle."""
        vm = await provisioner.acquire(requirements)
        try:
            assert vm.vm_id
            assert vm.host
            assert vm.provider == VMProvider.GCP

            active = await provisioner.list_active()
            vm_ids = [h.vm_id for h in active]
            assert vm.vm_id in vm_ids
        finally:
            await provisioner.release(vm)


class TestGCPAcquireAndSSH:
    @pytest.mark.timeout(600)
    async def test_acquire_wait_ssh_release(self, provisioner, requirements):
        """Acquire VM -> wait for SSH -> run command -> release."""
        vm = await provisioner.acquire(requirements)
        try:
            assert vm.provider == VMProvider.GCP
            assert vm.host

            conn = SSHConnection(
                SSHConfig(
                    host=vm.host,
                    user="tanren",
                    key_path="~/.ssh/gcp_tanren",
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
