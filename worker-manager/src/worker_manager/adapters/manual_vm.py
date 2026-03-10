"""Manual VM provisioner — pre-existing VMs from config list."""

from __future__ import annotations

import logging
from datetime import UTC, datetime
from typing import Any

from worker_manager.adapters.remote_types import VMHandle, VMRequirements

logger = logging.getLogger(__name__)


class NoVMAvailableError(Exception):
    """Raised when no VMs are available for assignment."""


class ManualVMProvisioner:
    """Provision from a pre-configured list of VMs.

    Implements the VMProvisioner protocol. Tracks which VMs are
    currently assigned using a VMStateStore. VMs are returned to
    the pool on release.
    """

    def __init__(
        self,
        vms: list[dict[str, Any]],
        state_store,
        provider: str = "manual",
    ) -> None:
        self._vms = vms
        self._state_store = state_store
        self._provider = provider

    async def acquire(self, requirements: VMRequirements) -> VMHandle:
        """Find first unassigned VM. Raises NoVMAvailableError if none available."""
        active = await self._state_store.get_active_assignments()
        active_vm_ids = {a.vm_id for a in active}

        for vm_config in self._vms:
            vm_id = str(vm_config["id"])
            if vm_id not in active_vm_ids:
                now = datetime.now(UTC).isoformat()
                raw_labels = vm_config.get("labels")
                labels: dict[str, str] = (
                    {str(k): str(v) for k, v in raw_labels.items()}
                    if isinstance(raw_labels, dict)
                    else {}
                )
                handle = VMHandle(
                    vm_id=vm_id,
                    host=str(vm_config["host"]),
                    provider=self._provider,
                    created_at=now,
                    labels=labels,
                )
                logger.info("Acquired VM %s at %s", vm_id, handle.host)
                return handle

        raise NoVMAvailableError(
            f"All {len(self._vms)} VMs are currently assigned"
        )

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
                if vm_config["id"] == assignment.vm_id:
                    handles.append(
                        VMHandle(
                            vm_id=assignment.vm_id,
                            host=assignment.host,
                            provider=self._provider,
                            created_at=assignment.assigned_at,
                        )
                    )
                    break
        return handles
