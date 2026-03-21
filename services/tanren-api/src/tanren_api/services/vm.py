"""VM service — list, provision, poll, release, and dry-run VMs."""

from __future__ import annotations

import asyncio
import contextlib
import logging
import uuid
from datetime import UTC, datetime

from tanren_api.errors import NotFoundError, ServiceError
from tanren_api.models import (
    ProvisionRequest,
    RunEnvironmentStatus,
    VMDryRunResult,
    VMProvisionAccepted,
    VMProvisionStatus,
    VMReleaseConfirmed,
    VMStatus,
    VMSummary,
)
from tanren_api.state import APIStateStore, EnvironmentRecord
from tanren_core.adapters.protocols import ExecutionEnvironment, VMStateStore
from tanren_core.adapters.remote_types import VMHandle, VMProvider, VMRequirements
from tanren_core.adapters.types import EnvironmentHandle, RemoteEnvironmentRuntime
from tanren_core.config import Config
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.remote_config import ProvisionerType, load_remote_config
from tanren_core.roles import RoleName
from tanren_core.roles_config import load_roles_config
from tanren_core.schemas import Dispatch, Outcome, Phase

logger = logging.getLogger(__name__)


def _now() -> str:
    return datetime.now(UTC).isoformat()


def _derive_provider(config: Config) -> VMProvider:
    """Derive VM provider from remote config.

    Returns:
        VMProvider: The provider type derived from configuration.

    Raises:
        ServiceError: If the remote config file cannot be loaded.
    """
    if not config.remote_config_path:
        return VMProvider.MANUAL
    try:
        remote_cfg = load_remote_config(config.remote_config_path)
    except Exception as exc:
        logger.exception("Failed to load remote config from %s", config.remote_config_path)
        raise ServiceError("Failed to load remote config") from exc
    if remote_cfg.provisioner.type == ProvisionerType.HETZNER:
        return VMProvider.HETZNER
    if remote_cfg.provisioner.type == ProvisionerType.GCP:
        return VMProvider.GCP
    return VMProvider.MANUAL


class VMService:
    """Service for VM lifecycle management."""

    def __init__(
        self,
        store: APIStateStore,
        config: Config | None = None,
        execution_env: ExecutionEnvironment | None = None,
        vm_state_store: VMStateStore | None = None,
    ) -> None:
        """Initialize with dependencies."""
        self._store = store
        self._config = config
        self._execution_env = execution_env
        self._vm_state_store = vm_state_store

    def _require_config(self) -> Config:
        """Return config or raise if unavailable.

        Returns:
            Config: The application configuration.

        Raises:
            ServiceError: If configuration is not set.
        """
        if self._config is None:
            raise ServiceError("Configuration unavailable — WM_* environment variables not set")
        return self._config

    async def list_vms(self) -> list[VMSummary]:
        """List active VM assignments.

        Returns:
            list[VMSummary]: Active VM assignments.
        """
        config = self._require_config()
        if self._vm_state_store is None:
            return []

        provider = _derive_provider(config)
        assignments = await self._vm_state_store.get_active_assignments()
        return [
            VMSummary(
                vm_id=a.vm_id,
                host=a.host,
                provider=provider,
                workflow_id=a.workflow_id,
                project=a.project,
                status=VMStatus.ACTIVE,
                created_at=a.assigned_at,
            )
            for a in assignments
        ]

    async def provision(self, body: ProvisionRequest) -> VMProvisionAccepted:
        """Provision a new VM (non-blocking).

        Returns:
            VMProvisionAccepted: Accepted response with tracking env_id.

        Raises:
            ServiceError: If remote execution environment is not configured.
        """
        config = self._require_config()
        if self._execution_env is None:
            raise ServiceError("Remote execution environment not configured")

        env_id = str(uuid.uuid4())
        execution_env = self._execution_env

        roles = load_roles_config(config.roles_config_path)
        resolved_tool = roles.resolve(RoleName.DEFAULT)

        dispatch = Dispatch(
            workflow_id=f"vm-provision-{body.project}-{uuid.uuid4().hex[:8]}",
            project=body.project,
            phase=Phase.DO_TASK,
            branch=body.branch,
            spec_folder="",
            cli=resolved_tool.cli,
            auth=resolved_tool.auth,
            timeout=1800,
            environment_profile=body.environment_profile,
            resolved_profile=EnvironmentProfile(name="default"),
        )

        record = EnvironmentRecord(
            env_id=env_id,
            handle=None,
            status=RunEnvironmentStatus.PROVISIONING,
            started_at=_now(),
        )
        await self._store.add_environment(record)

        async def _provision_background() -> None:
            handle: EnvironmentHandle | None = None
            try:
                handle = await execution_env.provision(dispatch, config)
                runtime = handle.runtime
                if not isinstance(runtime, RemoteEnvironmentRuntime):
                    raise ServiceError("Provisioned environment is not a remote runtime")
                vm_handle = runtime.vm_handle
                # Close SSH connection to prevent leak
                try:
                    close_fn = getattr(runtime.connection, "close", None)
                    if close_fn is not None:
                        await close_fn()
                except Exception:
                    logger.debug(
                        "Failed to close provision-time SSH connection for %s", vm_handle.vm_id
                    )
                updated = await self._store.try_transition_environment(
                    env_id,
                    from_statuses=frozenset({RunEnvironmentStatus.PROVISIONING}),
                    to_status=RunEnvironmentStatus.PROVISIONED,
                    handle=handle,
                    vm_id=vm_handle.vm_id,
                    host=vm_handle.host,
                    completed_at=_now(),
                )
                if updated is not None:
                    handle = None  # Persisted — suppress finally cleanup
            except asyncio.CancelledError:
                raise
            except Exception:
                handle = None  # Error handler owns cleanup
                logger.exception("VM provision failed for %s", env_id)
                await self._store.try_transition_environment(
                    env_id,
                    from_statuses=frozenset({RunEnvironmentStatus.PROVISIONING}),
                    to_status=RunEnvironmentStatus.FAILED,
                    outcome=Outcome.ERROR,
                    completed_at=_now(),
                )
            finally:
                if handle is not None:
                    logger.warning("Cleaning up orphaned VM provision for %s", env_id)
                    inner = asyncio.ensure_future(execution_env.teardown(handle))
                    try:
                        await asyncio.shield(inner)
                    except asyncio.CancelledError, Exception:
                        with contextlib.suppress(asyncio.CancelledError, Exception):
                            await inner

        task = asyncio.create_task(_provision_background())
        await self._store.update_environment(env_id, task=task)

        return VMProvisionAccepted(env_id=env_id)

    async def get_provision_status(self, env_id: str) -> VMProvisionStatus:
        """Poll status of an in-progress or completed VM provisioning.

        Returns:
            VMProvisionStatus: Current provisioning status.

        Raises:
            NotFoundError: If the provision env_id is not found.
        """
        config = self._require_config()
        record = await self._store.get_environment(env_id)
        if record is None:
            raise NotFoundError(f"Provision {env_id} not found")

        provider = _derive_provider(config)
        if record.status == RunEnvironmentStatus.PROVISIONED:
            vm_status = VMStatus.ACTIVE
        elif record.status == RunEnvironmentStatus.FAILED:
            vm_status = VMStatus.FAILED
        else:
            vm_status = VMStatus.PROVISIONING

        return VMProvisionStatus(
            env_id=env_id,
            status=vm_status,
            vm_id=record.vm_id or None,
            host=record.host or None,
            provider=provider if record.vm_id else None,
            created_at=record.started_at,
        )

    async def release(self, vm_id: str) -> VMReleaseConfirmed:
        """Release a VM assignment.

        Returns:
            VMReleaseConfirmed: Confirmation of the released VM.

        Raises:
            NotFoundError: If the VM is not found.
            ServiceError: If the execution environment is not configured or release fails.
        """
        config = self._require_config()
        if self._vm_state_store is None:
            raise NotFoundError(f"VM {vm_id} not found")

        assignment = await self._vm_state_store.get_assignment(vm_id)
        if assignment is None:
            raise NotFoundError(f"VM {vm_id} not found")

        if self._execution_env is None:
            raise ServiceError("Remote execution environment not configured")

        provider = _derive_provider(config)
        vm_handle = VMHandle(
            vm_id=assignment.vm_id,
            host=assignment.host,
            provider=provider,
            created_at=assignment.assigned_at,
        )
        try:
            await self._execution_env.release_vm(vm_handle)
        except Exception as exc:
            logger.exception("VM release failed for %s", vm_id)
            raise ServiceError("Failed to release VM") from exc

        await self._vm_state_store.record_release(vm_id)
        return VMReleaseConfirmed(vm_id=vm_id)

    async def dry_run(self, body: ProvisionRequest) -> VMDryRunResult:
        """Dry-run provision — show what would happen without creating resources.

        Returns:
            VMDryRunResult: Simulated provisioning result.
        """
        config = self._require_config()
        provider = _derive_provider(config)
        requirements = VMRequirements(profile=body.environment_profile)
        return VMDryRunResult(
            provider=provider,
            would_provision=self._execution_env is not None,
            requirements=requirements,
        )
