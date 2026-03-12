"""Tests for manual VM provisioner."""

from unittest.mock import AsyncMock

import pytest

from worker_manager.adapters.manual_vm import (
    ManualProvisionerSettings,
    ManualVMConfig,
    ManualVMProvisioner,
    NoVMAvailableError,
)
from worker_manager.adapters.remote_types import (
    VMAssignment,
    VMHandle,
    VMProvider,
    VMRequirements,
)


def _make_provisioner(
    vms: list[ManualVMConfig],
    active_assignments: list[VMAssignment] | None = None,
) -> ManualVMProvisioner:
    state_store = AsyncMock()
    state_store.get_active_assignments.return_value = active_assignments or []
    return ManualVMProvisioner(vms=vms, state_store=state_store)


REQUIREMENTS = VMRequirements(profile="default")

VM_A = ManualVMConfig(vm_id="vm-1", host="10.0.0.1")
VM_B = ManualVMConfig(vm_id="vm-2", host="10.0.0.2", labels={"gpu": "true"})


class TestAcquire:
    async def test_returns_handle_for_first_unassigned(self):
        provisioner = _make_provisioner(vms=[VM_A, VM_B])

        handle = await provisioner.acquire(REQUIREMENTS)

        assert isinstance(handle, VMHandle)
        assert handle.vm_id == "vm-1"
        assert handle.host == "10.0.0.1"
        assert handle.provider == VMProvider.MANUAL

    async def test_skips_already_assigned_vms(self):
        assigned = VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="spec",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00",
        )
        provisioner = _make_provisioner(vms=[VM_A, VM_B], active_assignments=[assigned])

        handle = await provisioner.acquire(REQUIREMENTS)

        assert handle.vm_id == "vm-2"
        assert handle.host == "10.0.0.2"

    async def test_raises_when_all_vms_assigned(self):
        assigned = [
            VMAssignment(
                vm_id="vm-1",
                workflow_id="wf-1",
                project="proj",
                spec="s",
                host="10.0.0.1",
                assigned_at="2026-01-01T00:00:00",
            ),
            VMAssignment(
                vm_id="vm-2",
                workflow_id="wf-2",
                project="proj",
                spec="s",
                host="10.0.0.2",
                assigned_at="2026-01-01T00:00:00",
            ),
        ]
        provisioner = _make_provisioner(vms=[VM_A, VM_B], active_assignments=assigned)

        with pytest.raises(NoVMAvailableError, match="All 2 VMs"):
            await provisioner.acquire(REQUIREMENTS)


class TestRelease:
    async def test_release_does_not_crash(self):
        provisioner = _make_provisioner(vms=[VM_A])
        handle = VMHandle(
            vm_id="vm-1",
            host="10.0.0.1",
            provider=VMProvider.MANUAL,
            created_at="2026-01-01T00:00:00",
        )

        await provisioner.release(handle)  # should not raise


class TestListActive:
    async def test_returns_handles_for_active_assignments(self):
        assigned = [
            VMAssignment(
                vm_id="vm-1",
                workflow_id="wf-1",
                project="proj",
                spec="s",
                host="10.0.0.1",
                assigned_at="2026-01-01T00:00:00",
            ),
        ]
        provisioner = _make_provisioner(vms=[VM_A, VM_B], active_assignments=assigned)

        handles = await provisioner.list_active()

        assert len(handles) == 1
        assert handles[0].vm_id == "vm-1"
        assert handles[0].host == "10.0.0.1"

    async def test_returns_empty_when_no_active(self):
        provisioner = _make_provisioner(vms=[VM_A, VM_B])

        handles = await provisioner.list_active()

        assert handles == []


class TestManualProvisionerSettings:
    def test_missing_vms_uses_empty_pool(self):
        parsed = ManualProvisionerSettings.from_settings({})

        assert parsed.vms == ()

    def test_rejects_non_list_vms(self):
        with pytest.raises(TypeError, match="must be a list of mappings"):
            ManualProvisionerSettings.from_settings({"vms": "not-a-list"})

    def test_rejects_non_mapping_entry(self):
        with pytest.raises(TypeError, match="item at index 1 is str"):
            ManualProvisionerSettings.from_settings(
                {"vms": [{"vm_id": "vm-1", "host": "10.0.0.1"}, "bad-entry"]}
            )

    def test_parses_valid_vm_entries(self):
        parsed = ManualProvisionerSettings.from_settings(
            {"vms": [{"id": "vm-1", "host": "10.0.0.1"}]}
        )

        assert len(parsed.vms) == 1
        assert parsed.vms[0].vm_id == "vm-1"
