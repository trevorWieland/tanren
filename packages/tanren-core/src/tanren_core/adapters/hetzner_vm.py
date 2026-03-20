"""Hetzner Cloud VM provisioner."""

from __future__ import annotations

import asyncio
import logging
import os
import time
from collections.abc import Mapping
from datetime import UTC, datetime
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict, Field, JsonValue

from tanren_core.adapters.remote_types import VMHandle, VMProvider, VMRequirements

if TYPE_CHECKING:
    from hcloud.servers.client import BoundServer


def _import_hcloud() -> type:
    """Import and return the hcloud Client class at runtime.

    Returns:
        The hcloud Client class.

    Raises:
        ImportError: If the hcloud package is not installed.
    """
    try:
        from hcloud import Client as _Client  # noqa: PLC0415 — optional dep

        return _Client
    except ImportError:
        raise ImportError(
            "hcloud is required for Hetzner provisioning. Install it with: uv sync --extra hetzner"
        ) from None


logger = logging.getLogger(__name__)


class HetznerProvisionerSettings(BaseModel):
    """Provider-owned settings for Hetzner VM provisioning."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    token_env: str = Field(default="HCLOUD_TOKEN")
    default_server_type: str = Field(...)
    location: str = Field(...)
    image: str = Field(...)
    architecture: str = Field(default="x86")
    ssh_key_name: str = Field(...)
    name_prefix: str = Field(default="tanren")
    labels: dict[str, str] = Field(default_factory=dict)
    managed_by_label_key: str = Field(default="managed-by")
    managed_by_label_value: str = Field(default="tanren")
    readiness_timeout_secs: int = Field(default=300, ge=10)
    poll_interval_secs: int = Field(default=2, ge=1)

    @classmethod
    def from_settings(cls, settings: Mapping[str, JsonValue]) -> HetznerProvisionerSettings:
        """Parse provider settings from remote.yml provisioner.settings.

        Returns:
            Validated HetznerProvisionerSettings.
        """
        return cls.model_validate(settings)


class HetznerVMProvisioner:
    """Provision VMs on Hetzner Cloud."""

    def __init__(self, settings: HetznerProvisionerSettings) -> None:
        """Initialize with Hetzner provisioner settings and validate prerequisites.

        Raises:
            ValueError: If the Hetzner API token environment variable is missing.
        """
        self._settings = settings
        token = os.environ.get(settings.token_env)
        if not token:
            raise ValueError(
                f"Missing Hetzner API token in environment variable: {settings.token_env}"
            )
        ClientCls = _import_hcloud()
        self._client = ClientCls(token=token)
        self._validate_prerequisites()

    def _validate_prerequisites(self) -> None:
        """Validate static settings against Hetzner resources.

        Raises:
            ValueError: If a configured location or SSH key is not found.
        """
        location = self._client.locations.get_by_name(self._settings.location)
        if location is None:
            raise ValueError(f"Hetzner location not found: {self._settings.location}")

        ssh_key = self._client.ssh_keys.get_by_name(self._settings.ssh_key_name)
        if ssh_key is None:
            raise ValueError(f"Hetzner SSH key not found: {self._settings.ssh_key_name}")

    def _create_server_sync(
        self, requirements: VMRequirements
    ) -> tuple[BoundServer, str, dict[str, str]]:
        """Resolve resources and create server (sync — call via to_thread).

        Returns:
            Tuple of (server, server_type_name, labels).

        Raises:
            ValueError: If a required Hetzner resource is not found.
        """
        server_type_name = requirements.server_type or self._settings.default_server_type
        server_type = self._client.server_types.get_by_name(server_type_name)
        if server_type is None:
            raise ValueError(f"Hetzner server type not found: {server_type_name}")

        location = self._client.locations.get_by_name(self._settings.location)
        if location is None:
            raise ValueError(f"Hetzner location not found: {self._settings.location}")

        ssh_key = self._client.ssh_keys.get_by_name(self._settings.ssh_key_name)
        if ssh_key is None:
            raise ValueError(f"Hetzner SSH key not found: {self._settings.ssh_key_name}")

        image = self._client.images.get_by_name_and_architecture(
            self._settings.image, self._settings.architecture
        )
        if image is None:
            raise ValueError(f"Hetzner image not found: {self._settings.image}")

        labels = dict(self._settings.labels)
        labels[self._settings.managed_by_label_key] = self._settings.managed_by_label_value
        labels["tanren-profile"] = requirements.profile

        server_name = (
            f"{self._settings.name_prefix}-{requirements.profile}-{int(time.time())}"
        ).replace("_", "-")

        create = self._client.servers.create(
            name=server_name,
            server_type=server_type,
            image=image,
            location=location,
            ssh_keys=[ssh_key],
            labels=labels,
        )
        return create.server, server_type_name, labels

    async def acquire(self, requirements: VMRequirements) -> VMHandle:
        """Create and wait for a Hetzner VM to become reachable.

        Returns:
            VMHandle for the provisioned VM.

        Raises:
            TimeoutError: If the VM does not become ready within the timeout.
        """
        server, server_type_name, labels = await asyncio.to_thread(
            self._create_server_sync, requirements
        )

        try:
            deadline = time.monotonic() + self._settings.readiness_timeout_secs
            while time.monotonic() < deadline:
                await asyncio.to_thread(server.reload)
                if self._is_running(server):
                    host = self._extract_public_ipv4(server)
                    if host:
                        hourly_cost = await asyncio.to_thread(
                            self._resolve_hourly_cost,
                            server_type_name,
                            self._settings.location,
                        )
                        return VMHandle(
                            vm_id=str(server.id),
                            host=host,
                            provider=VMProvider.HETZNER,
                            created_at=datetime.now(UTC).isoformat(),
                            labels=labels,
                            hourly_cost=hourly_cost,
                        )
                await asyncio.sleep(self._settings.poll_interval_secs)

            raise TimeoutError(
                f"Hetzner server {server.id} did not become ready "
                f"within {self._settings.readiness_timeout_secs}s"
            )
        except Exception:
            await asyncio.to_thread(
                self._delete_server_best_effort, server, context="acquire cleanup"
            )
            raise

    async def release(self, handle: VMHandle) -> None:
        """Delete the Hetzner server."""
        server = await asyncio.to_thread(self._get_server_for_handle, handle)
        if server is None:
            logger.warning("Hetzner release: server not found for %s", handle.vm_id)
            return
        await asyncio.to_thread(server.delete)

    async def list_active(self) -> list[VMHandle]:
        """List active tanren-managed Hetzner VMs.

        Returns:
            List of VMHandle for active VMs.
        """
        selector = f"{self._settings.managed_by_label_key}={self._settings.managed_by_label_value}"

        servers: list[BoundServer]
        try:
            servers = await asyncio.to_thread(self._client.servers.get_all, label_selector=selector)
        except TypeError:
            servers = await asyncio.to_thread(self._client.servers.get_all)

        handles: list[VMHandle] = []
        for server in servers:
            labels = dict(getattr(server, "labels", {}) or {})
            if (
                labels.get(self._settings.managed_by_label_key)
                != self._settings.managed_by_label_value
            ):
                continue
            host = self._extract_public_ipv4(server) or ""
            created_at_raw = getattr(server, "created", None)
            if hasattr(created_at_raw, "isoformat"):
                created_at = created_at_raw.isoformat()
            else:
                created_at = str(created_at_raw or datetime.now(UTC).isoformat())
            hourly_cost = await asyncio.to_thread(
                self._resolve_hourly_cost,
                self._server_type_name(server),
                self._settings.location,
            )
            handles.append(
                VMHandle(
                    vm_id=str(server.id),
                    host=host,
                    provider=VMProvider.HETZNER,
                    created_at=created_at,
                    labels=labels,
                    hourly_cost=hourly_cost,
                )
            )
        return handles

    def _get_server_for_handle(self, handle: VMHandle) -> BoundServer | None:
        server: BoundServer | None = None
        if handle.vm_id.isdigit():
            server = self._client.servers.get_by_id(int(handle.vm_id))
        if server is None:
            server = self._client.servers.get_by_name(handle.vm_id)
        return server

    def _delete_server_best_effort(self, server: BoundServer, *, context: str) -> None:
        try:
            server.delete()
        except Exception:
            logger.warning(
                "Hetzner %s: failed deleting server %s",
                context,
                server.id,
                exc_info=True,
            )

    def _resolve_hourly_cost(self, server_type_name: str, location_name: str) -> float | None:
        server_type = self._client.server_types.get_by_name(server_type_name)
        if server_type is None:
            return None
        prices = getattr(server_type, "prices", [])
        for price in prices:
            loc = self._price_location_name(price)
            if loc != location_name:
                continue
            hourly = self._price_hourly_value(price)
            if hourly is not None:
                return hourly
        for price in prices:
            hourly = self._price_hourly_value(price)
            if hourly is not None:
                return hourly
        return None

    @staticmethod
    def _is_running(
        server: object,
    ) -> bool:  # hcloud types are unstable; duck-typing via object is intentional
        status = str(getattr(server, "status", "")).lower()
        return status == "running"

    @staticmethod
    def _extract_public_ipv4(
        server: object,
    ) -> str | None:  # hcloud types are unstable; duck-typing via object is intentional
        public_net = getattr(server, "public_net", None)
        if public_net is None:
            return None

        ipv4_obj = getattr(public_net, "ipv4", None)
        if ipv4_obj is not None:
            ip = getattr(ipv4_obj, "ip", None)
            if isinstance(ip, str) and ip:
                return ip

        primary_ipv4 = getattr(public_net, "primary_ipv4", None)
        if primary_ipv4 is not None:
            ip = getattr(primary_ipv4, "ip", None)
            if isinstance(ip, str) and ip:
                return ip

        if isinstance(public_net, Mapping):
            raw_ipv4 = public_net.get("ipv4")
            if isinstance(raw_ipv4, Mapping):
                ip = raw_ipv4.get("ip")
                if isinstance(ip, str) and ip:
                    return ip
        return None

    @staticmethod
    def _price_location_name(
        price: object,
    ) -> str | None:  # hcloud types are unstable; duck-typing via object is intentional
        location = getattr(price, "location", None)
        if location is not None:
            loc_name = getattr(location, "name", None)
            if isinstance(loc_name, str):
                return loc_name
        if isinstance(price, Mapping):
            price_dict = {str(k): v for k, v in price.items()}
            location_raw = price_dict.get("location")
            if isinstance(location_raw, str):
                return location_raw
            if isinstance(location_raw, Mapping):
                location_dict = {str(k): v for k, v in location_raw.items()}
                loc_name = location_dict.get("name")
                if isinstance(loc_name, str):
                    return loc_name
        return None

    @staticmethod
    def _price_hourly_value(
        price: object,
    ) -> float | None:  # hcloud types are unstable; duck-typing via object is intentional
        def _as_float(
            value: object,
        ) -> float | None:  # hcloud types are unstable; duck-typing via object is intentional
            if not isinstance(value, str | int | float):
                return None
            try:
                return float(value)
            except TypeError, ValueError:
                return None

        price_hourly = getattr(price, "price_hourly", None)
        if price_hourly is not None:
            gross = getattr(price_hourly, "gross", None)
            if gross is not None:
                return _as_float(gross)
            net = getattr(price_hourly, "net", None)
            if net is not None:
                return _as_float(net)
        if isinstance(price, Mapping):
            price_dict = {str(k): v for k, v in price.items()}
            raw = price_dict.get("price_hourly")
            if isinstance(raw, Mapping):
                hourly_dict = {str(k): v for k, v in raw.items()}
                gross = hourly_dict.get("gross")
                if gross is not None:
                    return _as_float(gross)
                net = hourly_dict.get("net")
                if net is not None:
                    return _as_float(net)
        return None

    @staticmethod
    def _server_type_name(
        server: object,
    ) -> str:  # hcloud types are unstable; duck-typing via object is intentional
        st = getattr(server, "server_type", None)
        if st is None:
            return ""
        name = getattr(st, "name", None)
        if isinstance(name, str):
            return name
        return str(name or "")
