"""GCP Compute Engine VM provisioner."""

from __future__ import annotations

import asyncio
import logging
import os
import time
import types
from collections.abc import Mapping
from datetime import UTC, datetime
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict, Field, JsonValue

from tanren_core.adapters.remote_types import VMHandle, VMProvider, VMRequirements

if TYPE_CHECKING:
    from google.cloud.compute_v1.types import Instance


def _import_compute() -> types.ModuleType:
    """Import and return the google.cloud.compute_v1 module at runtime.

    Returns:
        The google.cloud.compute_v1 module.

    Raises:
        ImportError: If the google-cloud-compute package is not installed.
    """
    try:
        import google.cloud.compute_v1 as _compute  # noqa: PLC0415

        return _compute
    except ImportError:
        raise ImportError(
            "google-cloud-compute is required for GCP provisioning. "
            "Install it with: uv sync --extra gcp"
        ) from None


logger = logging.getLogger(__name__)


class GCPProvisionerSettings(BaseModel):
    """Provider-owned settings for GCP Compute Engine VM provisioning."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    project_id: str = Field(...)
    zone: str = Field(...)
    default_machine_type: str = Field(...)
    image_family: str = Field(...)
    image_project: str = Field(default="ubuntu-os-cloud")
    network: str = Field(default="default")
    subnet: str | None = Field(default=None)
    ssh_user: str = Field(default="tanren")
    ssh_key_env: str = Field(default="GCP_SSH_PUBLIC_KEY")
    service_account_email: str | None = Field(default=None)
    name_prefix: str = Field(default="tanren")
    labels: dict[str, str] = Field(default_factory=dict)
    managed_by_label_key: str = Field(default="managed-by")
    managed_by_label_value: str = Field(default="tanren")
    readiness_timeout_secs: int = Field(default=300, ge=10)
    poll_interval_secs: int = Field(default=5, ge=1)

    @classmethod
    def from_settings(cls, settings: Mapping[str, JsonValue]) -> GCPProvisionerSettings:
        """Parse provider settings from remote.yml provisioner.settings.

        Returns:
            Validated GCPProvisionerSettings.
        """
        return cls.model_validate(settings)


class GCPVMProvisioner:
    """Provision VMs on GCP Compute Engine."""

    def __init__(self, settings: GCPProvisionerSettings) -> None:
        """Initialize with GCP provisioner settings.

        Args:
            settings: Validated GCP provisioner settings.
        """
        self._settings = settings
        self._compute = _import_compute()
        self._instances_client = self._compute.InstancesClient()
        self._zone_ops_client = self._compute.ZoneOperationsClient()
        self._machine_types_client = self._compute.MachineTypesClient()

    async def acquire(self, requirements: VMRequirements) -> VMHandle:
        """Create and wait for a GCP VM to become reachable.

        Returns:
            VMHandle for the provisioned VM.

        Raises:
            TimeoutError: If the VM does not become ready within the timeout.
            ValueError: If the SSH public key environment variable is not set.
        """
        machine_type = requirements.server_type or self._settings.default_machine_type
        ssh_pub_key = os.environ.get(self._settings.ssh_key_env)
        if not ssh_pub_key:
            raise ValueError(
                f"Missing SSH public key in environment variable: {self._settings.ssh_key_env}"
            )

        labels = dict(self._settings.labels)
        labels[self._settings.managed_by_label_key] = self._settings.managed_by_label_value
        labels["tanren-profile"] = requirements.profile

        suffix = os.urandom(4).hex()
        instance_name = (
            f"{self._settings.name_prefix}-{requirements.profile}-{int(time.time())}-{suffix}"
        ).replace("_", "-")

        instance_resource = self._build_instance_resource(
            name=instance_name,
            machine_type=machine_type,
            labels=labels,
            ssh_user=self._settings.ssh_user,
            ssh_pub_key=ssh_pub_key,
        )

        operation = await asyncio.to_thread(
            self._instances_client.insert,
            project=self._settings.project_id,
            zone=self._settings.zone,
            instance_resource=instance_resource,
        )
        await asyncio.to_thread(operation.result)

        try:
            deadline = time.monotonic() + self._settings.readiness_timeout_secs
            while time.monotonic() < deadline:
                instance = await asyncio.to_thread(
                    self._instances_client.get,
                    project=self._settings.project_id,
                    zone=self._settings.zone,
                    instance=instance_name,
                )
                if str(instance.status) == "RUNNING":
                    host = self._extract_external_ip(instance)
                    if host:
                        return VMHandle(
                            vm_id=instance_name,
                            host=host,
                            provider=VMProvider.GCP,
                            created_at=datetime.now(UTC).isoformat(),
                            labels=labels,
                            hourly_cost=self._resolve_hourly_cost(),
                        )
                await asyncio.sleep(self._settings.poll_interval_secs)

            raise TimeoutError(
                f"GCP instance {instance_name} did not become ready "
                f"within {self._settings.readiness_timeout_secs}s"
            )
        except BaseException:
            await asyncio.to_thread(self._delete_instance_best_effort, instance_name)
            raise

    async def release(self, handle: VMHandle) -> None:
        """Delete a GCP VM instance.

        Args:
            handle: Handle of the VM to release.
        """
        try:
            operation = await asyncio.to_thread(
                self._instances_client.delete,
                project=self._settings.project_id,
                zone=self._settings.zone,
                instance=handle.vm_id,
            )
            await asyncio.to_thread(operation.result)
        except Exception:
            import sys  # noqa: PLC0415

            from google.api_core.exceptions import NotFound  # noqa: PLC0415

            exc = sys.exc_info()[1]
            if isinstance(exc, NotFound):
                logger.warning("GCP release: instance not found for %s", handle.vm_id)
                return
            raise

    async def list_active(self) -> list[VMHandle]:
        """List active tanren-managed GCP VM instances.

        Returns:
            List of VMHandle for active VMs.
        """
        filter_str = (
            f"labels.{self._settings.managed_by_label_key}={self._settings.managed_by_label_value}"
        )

        instances: list[Instance] = await asyncio.to_thread(
            lambda: list(
                self._instances_client.list(
                    project=self._settings.project_id,
                    zone=self._settings.zone,
                    filter=filter_str,
                )
            )
        )

        handles: list[VMHandle] = []
        for instance in instances:
            host = self._extract_external_ip(instance) or ""
            created_at = str(
                getattr(instance, "creation_timestamp", None) or datetime.now(UTC).isoformat()
            )
            labels = dict(instance.labels) if instance.labels else {}
            handles.append(
                VMHandle(
                    vm_id=instance.name,
                    host=host,
                    provider=VMProvider.GCP,
                    created_at=created_at,
                    labels=labels,
                    hourly_cost=self._resolve_hourly_cost(),
                )
            )
        return handles

    def _build_instance_resource(
        self,
        *,
        name: str,
        machine_type: str,
        labels: dict[str, str],
        ssh_user: str,
        ssh_pub_key: str,
    ) -> object:
        """Construct a compute_v1.Instance resource.

        Returns:
            A compute_v1.Instance object.
        """
        compute = self._compute
        zone = self._settings.zone

        boot_disk = compute.AttachedDisk(
            auto_delete=True,
            boot=True,
            initialize_params=compute.AttachedDiskInitializeParams(
                source_image=f"projects/{self._settings.image_project}/global/images/family/{self._settings.image_family}",
            ),
        )

        access_config = compute.AccessConfig(
            name="External NAT",
            type="ONE_TO_ONE_NAT",
        )

        network_interface = compute.NetworkInterface(
            network=f"projects/{self._settings.project_id}/global/networks/{self._settings.network}",
            access_configs=[access_config],
        )
        if self._settings.subnet:
            network_interface.subnetwork = (
                f"projects/{self._settings.project_id}/regions/"
                f"{zone.rsplit('-', 1)[0]}/subnetworks/{self._settings.subnet}"
            )

        metadata = compute.Metadata(
            items=[
                compute.Items(
                    key="ssh-keys",
                    value=f"{ssh_user}:{ssh_pub_key}",
                ),
            ],
        )

        service_accounts = []
        if self._settings.service_account_email:
            service_accounts.append(
                compute.ServiceAccount(
                    email=self._settings.service_account_email,
                    scopes=["https://www.googleapis.com/auth/cloud-platform"],
                )
            )

        return compute.Instance(
            name=name,
            machine_type=f"zones/{zone}/machineTypes/{machine_type}",
            disks=[boot_disk],
            network_interfaces=[network_interface],
            metadata=metadata,
            labels=labels,
            service_accounts=service_accounts,
        )

    @staticmethod
    def _extract_external_ip(instance: object) -> str | None:
        """Extract the external IP from an instance's first network interface.

        Returns:
            The external IP string, or None if not found.
        """
        interfaces = getattr(instance, "network_interfaces", None)
        if not interfaces:
            return None
        first = interfaces[0]
        access_configs = getattr(first, "access_configs", None)
        if not access_configs:
            return None
        ip = getattr(access_configs[0], "nat_i_p", None)
        if isinstance(ip, str) and ip:
            return ip
        return None

    @staticmethod
    def _resolve_hourly_cost() -> None:
        """Return hourly cost for the instance.

        Returns:
            Always None; GCP pricing requires the Cloud Billing API.
        """
        return None

    def _delete_instance_best_effort(self, instance_name: str) -> None:
        """Try to delete an instance, logging any errors.

        Args:
            instance_name: Name of the instance to delete.
        """
        try:
            op = self._instances_client.delete(
                project=self._settings.project_id,
                zone=self._settings.zone,
                instance=instance_name,
            )
            op.result()
        except Exception:
            logger.warning(
                "GCP cleanup: failed deleting instance %s",
                instance_name,
                exc_info=True,
            )
