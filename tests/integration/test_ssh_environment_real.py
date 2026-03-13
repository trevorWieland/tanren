"""Real SSH environment lifecycle tests — requires --ssh-host and --ssh-key."""

from __future__ import annotations

import pytest

from tanren_core.adapters.ssh import SSHConfig

pytestmark = pytest.mark.ssh


@pytest.fixture
def ssh_config(request):
    """Build SSHConfig from CLI options."""
    host = request.config.getoption("--ssh-host")
    key = request.config.getoption("--ssh-key")
    user = request.config.getoption("--ssh-user")
    if not host or not key:
        pytest.skip("--ssh-host and --ssh-key required")
    return SSHConfig(host=host, key_path=key, user=user)


async def test_real_provision_teardown_lifecycle(ssh_config):  # noqa: RUF029 — stub test
    """Provision an SSH environment, verify it's usable, then tear it down."""
    # TODO: instantiate SSHEnvironment, call provision(), run a health check,
    #       then call teardown() and confirm resources are released.
    pytest.skip("stub — implement when SSHEnvironment is wired up")


async def test_real_teardown_releases_vm_on_failure(ssh_config):  # noqa: RUF029 — stub test
    """Teardown must release the VM even if the worker process failed."""
    # TODO: provision, simulate a worker crash, call teardown(), and verify
    #       the remote process tree is cleaned up.
    pytest.skip("stub — implement when SSHEnvironment is wired up")
