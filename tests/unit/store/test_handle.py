"""Tests for PersistedEnvironmentHandle serialization."""

import pytest
from pydantic import ValidationError

from tanren_core.adapters.remote_types import VMProvider
from tanren_core.store.handle import (
    PersistedEnvironmentHandle,
    PersistedSSHConfig,
    PersistedVMInfo,
)


def _make_ssh_config(**overrides: object) -> PersistedSSHConfig:
    defaults: dict[str, object] = {"host": "10.0.0.1"}
    return PersistedSSHConfig.model_validate(defaults | overrides)


def _make_vm_info(**overrides: object) -> PersistedVMInfo:
    defaults: dict[str, object] = {
        "vm_id": "vm-123",
        "host": "10.0.0.1",
        "created_at": "2026-01-01T00:00:00Z",
    }
    return PersistedVMInfo.model_validate(defaults | overrides)


def _make_handle(**overrides: object) -> PersistedEnvironmentHandle:
    defaults: dict[str, object] = {
        "env_id": "env-abc",
        "worktree_path": "/workspace/myproject",
        "branch": "main",
        "project": "myproject",
        "provision_timestamp": "2026-01-01T00:00:00Z",
    }
    return PersistedEnvironmentHandle.model_validate(defaults | overrides)


class TestPersistedSSHConfig:
    def test_defaults(self) -> None:
        cfg = _make_ssh_config()
        assert cfg.user == "root"
        assert cfg.key_path == "~/.ssh/tanren_vm"
        assert cfg.port == 22
        assert cfg.connect_timeout == 10
        assert cfg.host_key_policy == "auto_add"

    def test_frozen(self) -> None:
        cfg = _make_ssh_config()
        with pytest.raises(ValidationError):
            cfg.host = "other"

    def test_roundtrip(self) -> None:
        cfg = _make_ssh_config(port=2222, user="tanren")
        restored = PersistedSSHConfig.model_validate_json(cfg.model_dump_json())
        assert restored == cfg


class TestPersistedVMInfo:
    def test_defaults(self) -> None:
        vm = _make_vm_info()
        assert vm.provider == VMProvider.MANUAL
        assert vm.labels == {}
        assert vm.hourly_cost is None

    def test_roundtrip(self) -> None:
        vm = _make_vm_info(
            provider=VMProvider.HETZNER,
            labels={"env": "staging"},
            hourly_cost=0.05,
        )
        restored = PersistedVMInfo.model_validate_json(vm.model_dump_json())
        assert restored == vm


class TestPersistedEnvironmentHandle:
    def test_local_handle(self) -> None:
        h = _make_handle(task_env={"FOO": "bar"})
        assert h.vm is None
        assert h.ssh_config is None
        assert h.task_env == {"FOO": "bar"}

    def test_remote_handle(self) -> None:
        h = _make_handle(
            vm=_make_vm_info(),
            ssh_config=_make_ssh_config(),
            workspace_remote_path="/workspace/myproject",
            teardown_commands=("rm -rf /tmp/test",),
            agent_user="tanren",
        )
        assert h.vm is not None
        assert h.vm.vm_id == "vm-123"
        assert h.ssh_config is not None
        assert h.ssh_config.host == "10.0.0.1"
        assert h.agent_user == "tanren"
        assert h.teardown_commands == ("rm -rf /tmp/test",)

    def test_roundtrip(self) -> None:
        h = _make_handle(
            vm=_make_vm_info(provider=VMProvider.GCP),
            ssh_config=_make_ssh_config(port=2222),
            workspace_remote_path="/workspace/proj",
            profile_name="remote",
        )
        restored = PersistedEnvironmentHandle.model_validate_json(h.model_dump_json())
        assert restored == h

    def test_required_fields(self) -> None:
        with pytest.raises(ValidationError):
            PersistedEnvironmentHandle(
                env_id="x",
                # missing worktree_path, branch, project, provision_timestamp
            )

    def test_extra_fields_forbidden(self) -> None:
        with pytest.raises(ValidationError):
            _make_handle(unexpected_field="bad")
