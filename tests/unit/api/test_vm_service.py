"""Tests for VMService."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from tanren_api.models import ProvisionRequest
from tanren_api.services.vm import VMService
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.store.factory import create_store

if TYPE_CHECKING:
    from pathlib import Path

    from tanren_core.store.repository import Store

import pytest

DEFAULT_PROFILE = EnvironmentProfile(name="default")


@pytest.fixture
async def store(tmp_path: Path):
    s = await create_store(str(tmp_path / "test.db"))
    yield s
    await s.close()


def _make_provision_request(**overrides: Any) -> ProvisionRequest:
    defaults: dict[str, Any] = {
        "project": "test",
        "branch": "main",
        "resolved_profile": DEFAULT_PROFILE,
    }
    return ProvisionRequest.model_validate(defaults | overrides)


class TestVMServiceProvision:
    async def test_provision_returns_env_id(self, store: Store) -> None:
        svc = VMService(event_store=store, job_queue=store, state_store=store)
        result = await svc.provision(_make_provision_request())
        assert result.env_id.startswith("vm-provision-test-")

    async def test_provision_passes_required_secrets(self, store: Store) -> None:
        svc = VMService(event_store=store, job_queue=store, state_store=store)
        body = _make_provision_request(required_secrets=("SECRET_A", "SECRET_B"))
        result = await svc.provision(body)

        view = await store.get_dispatch(result.env_id)
        assert view is not None
        assert view.dispatch.required_secrets == ("SECRET_A", "SECRET_B")

    async def test_provision_passes_cloud_secrets(self, store: Store) -> None:
        svc = VMService(event_store=store, job_queue=store, state_store=store)
        body = _make_provision_request(cloud_secrets={"CLOUD_KEY": "cloud_val"})
        result = await svc.provision(body)

        view = await store.get_dispatch(result.env_id)
        assert view is not None
        assert view.dispatch.cloud_secrets == {"CLOUD_KEY": "cloud_val"}

    async def test_provision_passes_project_env(self, store: Store) -> None:
        svc = VMService(event_store=store, job_queue=store, state_store=store)
        body = _make_provision_request(project_env={"DB_URL": "postgres://localhost"})
        result = await svc.provision(body)

        view = await store.get_dispatch(result.env_id)
        assert view is not None
        assert view.dispatch.project_env == {"DB_URL": "postgres://localhost"}
