"""Manual VM provisioner — pre-existing VMs from config list."""

from __future__ import annotations

import logging
from datetime import UTC, datetime

from worker_manager.adapters.remote_types import VMHandle, VMRequirements
from worker_manager.remote_config import RemoteVMConfig

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
        vms: list[RemoteVMConfig],
        state_store,
    ) -> None:
        self._vms = vms
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
                    provider=vm_config.provider,
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
                            provider=vm_config.provider,
                            created_at=assignment.assigned_at,
                            labels=vm_config.labels,
                            hourly_cost=vm_config.hourly_cost,
                        )
                    )
                    break
        return handles
