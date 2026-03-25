"""Tests for GCP Compute Engine VM provisioner."""

from __future__ import annotations

from datetime import UTC, datetime
from types import SimpleNamespace
from unittest.mock import Mock

import pytest

from tanren_core.adapters.gcp_vm import GCPProvisionerSettings, GCPVMProvisioner
from tanren_core.adapters.remote_types import VMHandle, VMProvider, VMRequirements


class _FakeInstance:
    def __init__(
        self,
        *,
        name="test-vm",
        statuses=None,
        ip="203.0.113.10",
        internal_ip="10.128.0.2",
        has_access_config=True,
        labels=None,
    ):
        self.name = name
        self.labels = labels or {}
        self.creation_timestamp = datetime.now(UTC).isoformat()
        self._statuses = statuses or ["RUNNING"]
        self._idx = 0
        self.status = self._statuses[0]
        access_configs = [SimpleNamespace(nat_i_p=ip)] if has_access_config else []
        ni = SimpleNamespace(access_configs=access_configs, network_i_p=internal_ip)
        self.network_interfaces = [ni]


def _build_compute_module(instances_client):
    """Build a fake compute_v1 module."""
    mod = SimpleNamespace(
        InstancesClient=Mock(return_value=instances_client),
        ZoneOperationsClient=Mock(return_value=Mock()),
        MachineTypesClient=Mock(return_value=Mock()),
        Instance=lambda **kw: SimpleNamespace(**kw),
        AttachedDisk=lambda **kw: SimpleNamespace(**kw),
        AttachedDiskInitializeParams=lambda **kw: SimpleNamespace(**kw),
        AccessConfig=lambda **kw: SimpleNamespace(**kw),
        NetworkInterface=lambda **kw: SimpleNamespace(**kw),
        Metadata=lambda **kw: SimpleNamespace(**kw),
        Items=lambda **kw: SimpleNamespace(**kw),
        ServiceAccount=lambda **kw: SimpleNamespace(**kw),
        ListInstancesRequest=lambda **kw: SimpleNamespace(**kw),
    )
    return mod


def _settings(**overrides):
    return GCPProvisionerSettings(
        project_id=overrides.get("project_id", "my-project"),
        zone=overrides.get("zone", "us-central1-a"),
        default_machine_type=overrides.get("default_machine_type", "e2-standard-4"),
        image_family=overrides.get("image_family", "ubuntu-2404-lts-amd64"),
        enable_external_ip=overrides.get("enable_external_ip", True),
        boot_disk_size_gb=overrides.get("boot_disk_size_gb", 50),
        boot_disk_type=overrides.get("boot_disk_type", "pd-balanced"),
        readiness_timeout_secs=overrides.get("readiness_timeout_secs", 300),
        poll_interval_secs=overrides.get("poll_interval_secs", 5),
    )


def _make_provisioner(monkeypatch, instances_client, **settings_kw):
    mod = _build_compute_module(instances_client)
    monkeypatch.setattr("tanren_core.adapters.gcp_vm._import_compute", lambda: mod)
    return GCPVMProvisioner(_settings(**settings_kw))


@pytest.mark.asyncio
async def test_acquire_creates_instance_with_expected_params(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")
    fake = _FakeInstance()
    insert_op = Mock()
    insert_op.result = Mock(return_value=None)
    client = Mock()
    client.insert = Mock(return_value=insert_op)
    client.get = Mock(return_value=fake)

    provisioner = _make_provisioner(monkeypatch, client)
    handle = await provisioner.acquire(
        VMRequirements(profile="default", server_type="e2-standard-4")
    )

    assert handle.provider == VMProvider.GCP
    insert_kwargs = client.insert.call_args.kwargs
    assert insert_kwargs["project"] == "my-project"
    assert insert_kwargs["zone"] == "us-central1-a"
    resource = insert_kwargs["instance_resource"]
    assert "e2-standard-4" in resource.machine_type
    assert resource.labels["managed-by"] == "tanren"


@pytest.mark.asyncio
async def test_acquire_extracts_external_ip(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")
    fake = _FakeInstance(ip="34.120.0.1")
    insert_op = Mock()
    insert_op.result = Mock(return_value=None)
    client = Mock()
    client.insert = Mock(return_value=insert_op)
    client.get = Mock(return_value=fake)

    provisioner = _make_provisioner(monkeypatch, client)
    handle = await provisioner.acquire(VMRequirements(profile="default"))

    assert handle.host == "34.120.0.1"


@pytest.mark.asyncio
async def test_acquire_times_out_when_instance_not_ready(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")

    def fake_get(**_kw):
        return _FakeInstance(statuses=["PROVISIONING"])

    insert_op = Mock()
    insert_op.result = Mock(return_value=None)
    client = Mock()
    client.insert = Mock(return_value=insert_op)
    client.get = fake_get
    client.delete = Mock(return_value=Mock(result=Mock(return_value=None)))

    provisioner = _make_provisioner(
        monkeypatch, client, readiness_timeout_secs=10, poll_interval_secs=10
    )

    with pytest.raises(TimeoutError):
        await provisioner.acquire(VMRequirements(profile="default"))

    client.delete.assert_called_once()


@pytest.mark.asyncio
async def test_release_deletes_instance(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")
    fake = _FakeInstance()
    insert_op = Mock()
    insert_op.result = Mock(return_value=None)
    delete_op = Mock()
    delete_op.result = Mock(return_value=None)
    client = Mock()
    client.insert = Mock(return_value=insert_op)
    client.get = Mock(return_value=fake)
    client.delete = Mock(return_value=delete_op)

    provisioner = _make_provisioner(monkeypatch, client)
    handle = await provisioner.acquire(VMRequirements(profile="default"))
    await provisioner.release(handle)

    client.delete.assert_called_once()
    delete_kwargs = client.delete.call_args.kwargs
    assert delete_kwargs["project"] == "my-project"
    assert delete_kwargs["instance"] == handle.vm_id


@pytest.mark.asyncio
async def test_release_missing_instance_is_graceful(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")
    from google.api_core.exceptions import (
        NotFound,
    )

    client = Mock()
    client.delete = Mock(side_effect=NotFound("not found"))

    provisioner = _make_provisioner(monkeypatch, client)

    handle = _make_handle("missing-vm")
    await provisioner.release(handle)


@pytest.mark.asyncio
async def test_release_raises_when_delete_fails(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")

    client = Mock()
    client.delete = Mock(side_effect=RuntimeError("API error"))

    provisioner = _make_provisioner(monkeypatch, client)

    handle = _make_handle("fail-vm")
    with pytest.raises(RuntimeError, match="API error"):
        await provisioner.release(handle)


@pytest.mark.asyncio
async def test_list_active_filters_by_managed_label(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")

    managed = _FakeInstance(name="managed-1", labels={"managed-by": "tanren"})

    client = Mock()
    client.list = Mock(return_value=[managed])

    provisioner = _make_provisioner(monkeypatch, client)
    handles = await provisioner.list_active()

    assert len(handles) == 1
    assert handles[0].vm_id == "managed-1"
    assert handles[0].provider == VMProvider.GCP
    request = client.list.call_args.kwargs["request"]
    assert "managed-by=tanren" in request.filter


def test_missing_gcp_dependency_error_is_clear(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA_test")

    def _raise():
        raise ImportError(
            "google-cloud-compute is required for GCP provisioning. "
            "Install it with: uv sync --extra gcp"
        )

    monkeypatch.setattr("tanren_core.adapters.gcp_vm._import_compute", _raise)

    with pytest.raises(ImportError, match="uv sync --extra gcp"):
        GCPVMProvisioner(_settings())


def test_settings_from_remote_yml():
    raw = {
        "project_id": "my-project",
        "zone": "us-central1-a",
        "default_machine_type": "e2-standard-4",
        "image_family": "ubuntu-2404-lts-amd64",
        "image_project": "ubuntu-os-cloud",
        "ssh_user": "agent",
        "name_prefix": "test",
        "labels": {"env": "ci"},
        "enable_external_ip": False,
        "boot_disk_size_gb": 100,
        "boot_disk_type": "pd-ssd",
    }
    settings = GCPProvisionerSettings.from_settings(raw)
    assert settings.project_id == "my-project"
    assert settings.zone == "us-central1-a"
    assert settings.ssh_user == "agent"
    assert settings.labels == {"env": "ci"}
    assert settings.image_project == "ubuntu-os-cloud"
    assert settings.enable_external_ip is False
    assert settings.boot_disk_size_gb == 100
    assert settings.boot_disk_type == "pd-ssd"


def test_default_settings_match_expected_values():
    settings = _settings()
    assert settings.enable_external_ip is True
    assert settings.boot_disk_size_gb == 50
    assert settings.boot_disk_type == "pd-balanced"


@pytest.mark.asyncio
async def test_acquire_private_vpc_uses_internal_ip(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")
    fake = _FakeInstance(has_access_config=False, internal_ip="10.128.0.5")
    insert_op = Mock()
    insert_op.result = Mock(return_value=None)
    client = Mock()
    client.insert = Mock(return_value=insert_op)
    client.get = Mock(return_value=fake)

    provisioner = _make_provisioner(monkeypatch, client, enable_external_ip=False)
    handle = await provisioner.acquire(VMRequirements(profile="default"))

    assert handle.host == "10.128.0.5"
    # Verify no AccessConfig was attached
    resource = client.insert.call_args.kwargs["instance_resource"]
    assert resource.network_interfaces[0].access_configs == []


@pytest.mark.asyncio
async def test_acquire_external_ip_attaches_access_config(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")
    fake = _FakeInstance(ip="34.120.0.1")
    insert_op = Mock()
    insert_op.result = Mock(return_value=None)
    client = Mock()
    client.insert = Mock(return_value=insert_op)
    client.get = Mock(return_value=fake)

    provisioner = _make_provisioner(monkeypatch, client, enable_external_ip=True)
    handle = await provisioner.acquire(VMRequirements(profile="default"))

    assert handle.host == "34.120.0.1"
    resource = client.insert.call_args.kwargs["instance_resource"]
    ac_list = resource.network_interfaces[0].access_configs
    assert len(ac_list) == 1
    assert ac_list[0].name == "External NAT"


@pytest.mark.asyncio
async def test_acquire_waits_for_external_ip_when_enabled(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")
    # First poll: RUNNING but external IP not yet assigned; second poll: IP available
    no_ip = _FakeInstance(ip="", internal_ip="10.128.0.9")
    with_ip = _FakeInstance(ip="34.120.0.2", internal_ip="10.128.0.9")
    insert_op = Mock()
    insert_op.result = Mock(return_value=None)
    client = Mock()
    client.insert = Mock(return_value=insert_op)
    client.get = Mock(side_effect=[no_ip, with_ip])

    provisioner = _make_provisioner(
        monkeypatch, client, enable_external_ip=True, poll_interval_secs=1
    )
    handle = await provisioner.acquire(VMRequirements(profile="default"))

    # Must have polled twice and returned the external IP, not the internal one
    assert handle.host == "34.120.0.2"
    assert client.get.call_count == 2


def test_boot_disk_size_and_type_in_instance_resource(monkeypatch):
    monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA testkey")
    client = Mock()
    provisioner = _make_provisioner(
        monkeypatch, client, boot_disk_size_gb=200, boot_disk_type="pd-ssd"
    )

    resource = provisioner._build_instance_resource(
        name="test-vm",
        machine_type="e2-standard-4",
        labels={},
        ssh_user="tanren",
        ssh_pub_key="ssh-ed25519 AAAA testkey",
    )

    boot_disk = resource.disks[0]
    assert boot_disk.initialize_params.disk_size_gb == 200
    assert "pd-ssd" in boot_disk.initialize_params.disk_type


def _make_handle(vm_id):
    return VMHandle(
        vm_id=vm_id,
        host="10.0.0.1",
        provider=VMProvider.GCP,
        created_at=datetime.now(UTC).isoformat(),
    )
