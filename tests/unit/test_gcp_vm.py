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
        labels=None,
    ):
        self.name = name
        self.labels = labels or {}
        self.creation_timestamp = datetime.now(UTC).isoformat()
        self._statuses = statuses or ["RUNNING"]
        self._idx = 0
        self.status = self._statuses[0]
        ac = SimpleNamespace(nat_i_p=ip)
        ni = SimpleNamespace(access_configs=[ac])
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
    gae = pytest.importorskip("google.api_core.exceptions")

    client = Mock()
    client.delete = Mock(side_effect=gae.NotFound("not found"))

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
    }
    settings = GCPProvisionerSettings.from_settings(raw)
    assert settings.project_id == "my-project"
    assert settings.zone == "us-central1-a"
    assert settings.ssh_user == "agent"
    assert settings.labels == {"env": "ci"}
    assert settings.image_project == "ubuntu-os-cloud"


def _make_handle(vm_id):
    return VMHandle(
        vm_id=vm_id,
        host="10.0.0.1",
        provider=VMProvider.GCP,
        created_at=datetime.now(UTC).isoformat(),
    )
