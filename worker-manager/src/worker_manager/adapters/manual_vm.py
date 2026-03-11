"""Manual VM provisioner — pre-existing VMs from config list."""

from __future__ import annotations

import logging
from collections.abc import Mapping
from datetime import UTC, datetime

from pydantic import BaseModel, ConfigDict, Field, JsonValue

from worker_manager.adapters.remote_types import VMHandle, VMProvider, VMRequirements

logger = logging.getLogger(__name__)


class NoVMAvailableError(Exception):
    """Raised when no VMs are available for assignment."""


class ManualVMConfig(BaseModel):
    """Typed VM entry for manual VM pools."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vm_id: str = Field(...)
    host: str = Field(...)
    labels: dict[str, str] = Field(default_factory=dict)
    metadata: dict[str, str] = Field(default_factory=dict)
    hourly_cost: float | None = Field(default=None, ge=0.0)

    @classmethod
    def from_raw(cls, raw: Mapping[str, JsonValue]) -> ManualVMConfig:
        """Build from raw settings, supporting 'id' alias."""
        data = dict(raw)
        if "vm_id" not in data and "id" in data:
            data["vm_id"] = data.pop("id")
        return cls.model_validate(data)


class ManualProvisionerSettings(BaseModel):
    """Provider-owned settings for manual VM provisioning."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vms: tuple[ManualVMConfig, ...] = Field(default_factory=tuple)

    @classmethod
    def from_settings(cls, settings: Mapping[str, JsonValue]) -> ManualProvisionerSettings:
        """Parse provider settings from remote.yml provisioner.settings."""
        raw_vms = settings.get("vms", [])
        vm_entries: list[ManualVMConfig] = []
        if isinstance(raw_vms, list):
            for vm in raw_vms:
                if isinstance(vm, Mapping):
                    vm_entries.append(ManualVMConfig.from_raw(vm))
        return cls(vms=tuple(vm_entries))


class ManualVMProvisioner:
    """Provision from a pre-configured list of VMs.

    Implements the VMProvisioner protocol. Tracks which VMs are
    currently assigned using a VMStateStore. VMs are returned to
    the pool on release.
    """

    def __init__(
        self,
        vms: list[ManualVMConfig] | tuple[ManualVMConfig, ...],
        state_store,
    ) -> None:
        self._vms = list(vms)
        self._state_store = state_store

    async def acquire(self, requirements: VMRequirements) -> VMHandle:
        """Find first unassigned VM. Raises NoVMAvailableError if none available."""
        active = await self._state_store.get_active_assignments()
        active_vm_ids = {a.vm_id for a in active}

        for vm_config in self._vms:
            vm_id = vm_config.vm_id
            if vm_id not in active_vm_ids:
                now = datetime.now(UTC).isoformat()
                handle = VMHandle(
                    vm_id=vm_id,
                    host=vm_config.host,
                    provider=VMProvider.MANUAL,
                    created_at=now,
                    labels=vm_config.labels,
                    hourly_cost=vm_config.hourly_cost,
                )
                logger.info("Acquired VM %s at %s", vm_id, handle.host)
                return handle

        raise NoVMAvailableError(f"All {len(self._vms)} VMs are currently assigned")

    async def release(self, handle: VMHandle) -> None:
        """Release a VM back to the pool."""
        logger.info("Released VM %s at %s", handle.vm_id, handle.host)

    async def list_active(self) -> list[VMHandle]:
        """List currently assigned VMs."""
        active = await self._state_store.get_active_assignments()
        handles = []
        for assignment in active:
            # Find the VM config for this assignment
            for vm_config in self._vms:
                if vm_config.vm_id == assignment.vm_id:
                    handles.append(
                        VMHandle(
                            vm_id=assignment.vm_id,
                            host=assignment.host,
                            provider=VMProvider.MANUAL,
                            created_at=assignment.assigned_at,
                            labels=vm_config.labels,
                            hourly_cost=vm_config.hourly_cost,
                        )
                    )
                    break
        return handles
