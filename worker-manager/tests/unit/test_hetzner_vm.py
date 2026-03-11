"""Tests for Hetzner VM provisioner."""

from __future__ import annotations

from datetime import UTC, datetime
from types import SimpleNamespace
from unittest.mock import Mock

import pytest

from worker_manager.adapters.hetzner_vm import (
    HetznerProvisionerSettings,
    HetznerVMProvisioner,
)
from worker_manager.adapters.remote_types import VMHandle, VMProvider, VMRequirements


class _FakeServer:
    def __init__(
        self,
        *,
        server_id: int = 101,
        name: str = "srv",
        statuses: list[str] | None = None,
        ips: list[str | None] | None = None,
        labels: dict[str, str] | None = None,
    ) -> None:
        self.id = server_id
        self.name = name
        self.labels = labels or {}
        self.server_type = SimpleNamespace(name="cpx31")
        self.created = datetime.now(UTC)
        self._statuses = statuses or ["running"]
        self._ips = ips or ["203.0.113.10"]
        self._idx = 0
        self.status = self._statuses[0]
        self.public_net = SimpleNamespace(ipv4=SimpleNamespace(ip=self._ips[0]))
        self.delete = Mock()

    def reload(self) -> None:
        if self._idx + 1 < len(self._statuses):
            self._idx += 1
        self.status = self._statuses[self._idx]
        ip = self._ips[self._idx] if self._idx < len(self._ips) else None
        self.public_net = SimpleNamespace(ipv4=SimpleNamespace(ip=ip))


class _FakeServers:
    def __init__(self, server: _FakeServer):
        self._server = server
        self.create = Mock(return_value=SimpleNamespace(server=server))
        self.get_all = Mock(return_value=[server])
        self.get_by_id = Mock(return_value=server)
        self.get_by_name = Mock(return_value=server)


def _build_client(server: _FakeServer):
    location = SimpleNamespace(name="ash")
    ssh_key = SimpleNamespace(name="default")
    prices = [
        SimpleNamespace(
            location=SimpleNamespace(name="ash"),
            price_hourly=SimpleNamespace(gross="0.051"),
        )
    ]
    server_type = SimpleNamespace(name="cpx31", prices=prices)
    servers = _FakeServers(server)
    return SimpleNamespace(
        locations=SimpleNamespace(get_by_name=Mock(return_value=location)),
        ssh_keys=SimpleNamespace(get_by_name=Mock(return_value=ssh_key)),
        images=SimpleNamespace(get_by_name=Mock(return_value=SimpleNamespace(name="ubuntu-24.04"))),
        server_types=SimpleNamespace(get_by_name=Mock(return_value=server_type)),
        servers=servers,
    )


def _settings() -> HetznerProvisionerSettings:
    return HetznerProvisionerSettings(
        token_env="HCLOUD_TOKEN",
        default_server_type="cpx31",
        location="ash",
        image="ubuntu-24.04",
        ssh_key_name="default",
        managed_by_label_key="managed-by",
        managed_by_label_value="tanren",
    )


@pytest.mark.asyncio
async def test_acquire_creates_server_with_expected_params(monkeypatch):
    monkeypatch.setenv("HCLOUD_TOKEN", "tok")
    server = _FakeServer(statuses=["off", "running"], ips=[None, "203.0.113.10"])
    client = _build_client(server)
    monkeypatch.setattr(
        "worker_manager.adapters.hetzner_vm._build_hcloud_client",
        lambda token: client,
    )

    provisioner = HetznerVMProvisioner(_settings())
    handle = await provisioner.acquire(VMRequirements(profile="default", server_type="cpx31"))

    assert handle.provider == VMProvider.HETZNER
    assert handle.host == "203.0.113.10"
    assert handle.hourly_cost == pytest.approx(0.051)
    create_kwargs = client.servers.create.call_args.kwargs
    assert create_kwargs["server_type"].name == "cpx31"
    assert create_kwargs["location"].name == "ash"
    assert create_kwargs["image"].name == "ubuntu-24.04"
    assert create_kwargs["labels"]["managed-by"] == "tanren"


@pytest.mark.asyncio
async def test_acquire_times_out_when_server_not_ready(monkeypatch):
    monkeypatch.setenv("HCLOUD_TOKEN", "tok")
    server = _FakeServer(statuses=["off", "off", "off"], ips=[None, None, None])
    client = _build_client(server)
    monkeypatch.setattr(
        "worker_manager.adapters.hetzner_vm._build_hcloud_client",
        lambda token: client,
    )
    settings = _settings().model_copy(
        update={"readiness_timeout_secs": 1, "poll_interval_secs": 1}
    )
    provisioner = HetznerVMProvisioner(settings)

    with pytest.raises(TimeoutError):
        await provisioner.acquire(VMRequirements(profile="default"))


def test_init_raises_when_ssh_key_missing(monkeypatch):
    monkeypatch.setenv("HCLOUD_TOKEN", "tok")
    server = _FakeServer()
    client = _build_client(server)
    client.ssh_keys.get_by_name.return_value = None
    monkeypatch.setattr(
        "worker_manager.adapters.hetzner_vm._build_hcloud_client",
        lambda token: client,
    )

    with pytest.raises(ValueError, match="SSH key"):
        HetznerVMProvisioner(_settings())


@pytest.mark.asyncio
async def test_release_deletes_server(monkeypatch):
    monkeypatch.setenv("HCLOUD_TOKEN", "tok")
    server = _FakeServer()
    client = _build_client(server)
    monkeypatch.setattr(
        "worker_manager.adapters.hetzner_vm._build_hcloud_client",
        lambda token: client,
    )
    provisioner = HetznerVMProvisioner(_settings())

    handle = await provisioner.acquire(VMRequirements(profile="default"))
    await provisioner.release(handle)

    server.delete.assert_called_once()


@pytest.mark.asyncio
async def test_release_missing_server_is_graceful(monkeypatch):
    monkeypatch.setenv("HCLOUD_TOKEN", "tok")
    server = _FakeServer()
    client = _build_client(server)
    client.servers.get_by_id.return_value = None
    client.servers.get_by_name.return_value = None
    monkeypatch.setattr(
        "worker_manager.adapters.hetzner_vm._build_hcloud_client",
        lambda token: client,
    )
    provisioner = HetznerVMProvisioner(_settings())

    await provisioner.release(
        VMHandle(
            vm_id="999",
            host="203.0.113.10",
            provider=VMProvider.HETZNER,
            created_at=datetime.now(UTC).isoformat(),
        )
    )


@pytest.mark.asyncio
async def test_list_active_filters_by_managed_label(monkeypatch):
    monkeypatch.setenv("HCLOUD_TOKEN", "tok")
    managed = _FakeServer(labels={"managed-by": "tanren"})
    unmanaged = _FakeServer(server_id=202, labels={"managed-by": "other"})
    client = _build_client(managed)
    client.servers.get_all.return_value = [managed, unmanaged]
    monkeypatch.setattr(
        "worker_manager.adapters.hetzner_vm._build_hcloud_client",
        lambda token: client,
    )
    provisioner = HetznerVMProvisioner(_settings())

    handles = await provisioner.list_active()

    assert len(handles) == 1
    assert handles[0].vm_id == str(managed.id)


def test_missing_hcloud_dependency_error_is_clear(monkeypatch):
    monkeypatch.setenv("HCLOUD_TOKEN", "tok")
    monkeypatch.setattr(
        "worker_manager.adapters.hetzner_vm._build_hcloud_client",
        lambda token: (_ for _ in ()).throw(RuntimeError("Install with: uv sync --extra hetzner")),
    )

    with pytest.raises(RuntimeError, match="uv sync --extra hetzner"):
        HetznerVMProvisioner(_settings())
