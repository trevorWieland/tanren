"""Tests for the shared SSH execution environment builder."""

from __future__ import annotations

from types import SimpleNamespace
from typing import TYPE_CHECKING
from unittest.mock import Mock

import pytest
from pydantic import ValidationError

from tanren_core.adapters.remote_types import VMProvider
from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment
from tanren_core.builder import (
    build_execution_environment,
    build_ssh_execution_environment,
    validate_provisioner_requirements,
)
from tanren_core.env.environment_schema import (
    DispatchProvisionerConfig,
    DockerExecutionConfig,
    EnvironmentProfile,
    EnvironmentProfileType,
    RemoteExecutionConfig,
)
from tanren_core.remote_config import ProvisionerType
from tanren_core.worker_config import WorkerConfig

if TYPE_CHECKING:
    from pathlib import Path

_ROLES_YML = """\
agents:
  default:
    cli: claude
    auth: subscription
    model: opus
"""


def _make_config(tmp_path: Path) -> WorkerConfig:
    roles_path = tmp_path / "roles.yml"
    roles_path.write_text(_ROLES_YML)
    return WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        db_url=str(tmp_path / "events.db"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        roles_config_path=str(roles_path),
    )


_MANUAL_PROVISIONER = DispatchProvisionerConfig(
    type="manual",
    settings={"vms": [{"id": "vm-1", "host": "10.0.0.1"}]},
)


def _manual_remote_cfg(**overrides: object) -> RemoteExecutionConfig:
    """Build a minimal RemoteExecutionConfig with a manual provisioner."""
    return RemoteExecutionConfig.model_validate({
        "provisioner": _MANUAL_PROVISIONER,
        "repo_url": "https://github.com/test/test.git",
        "required_clis": ("claude",),
        **overrides,
    })


class TestBuildSSHExecutionEnvironment:
    def test_build_manual_provisioner(self, tmp_path):
        config = _make_config(tmp_path)
        remote_cfg = _manual_remote_cfg()
        env, state_store = build_ssh_execution_environment(config, remote_cfg)

        assert isinstance(env, SSHExecutionEnvironment)
        assert state_store is not None

    def test_build_manual_provisioner_sets_provider(self, tmp_path):
        config = _make_config(tmp_path)
        remote_cfg = _manual_remote_cfg()
        env, _ = build_ssh_execution_environment(config, remote_cfg)

        assert env._provider == VMProvider.MANUAL

    def test_build_unsupported_provisioner_raises(self, tmp_path):
        config = _make_config(tmp_path)
        remote_cfg = RemoteExecutionConfig(
            provisioner=DispatchProvisionerConfig(type="aws", settings={}),
            repo_url="https://github.com/test/test.git",
        )
        with pytest.raises((ValueError, ValidationError)):
            build_ssh_execution_environment(config, remote_cfg)

    def test_build_uses_required_clis_for_credential_providers(self, tmp_path):
        """Builder uses remote_cfg.required_clis to determine credential providers."""
        config = _make_config(tmp_path)
        remote_cfg = _manual_remote_cfg(required_clis=("claude", "codex"))
        env, _ = build_ssh_execution_environment(config, remote_cfg)

        provider_names = sorted(p.name for p in env._credential_providers)
        assert provider_names == ["claude", "codex"]

    def test_build_sets_agent_user(self, tmp_path):
        """Builder configures agent_user on SSHExecutionEnvironment."""
        config = _make_config(tmp_path)
        remote_cfg = _manual_remote_cfg()
        env, _ = build_ssh_execution_environment(config, remote_cfg)

        assert env._agent_user == "tanren"

    def test_build_gcp_provisioner(self, tmp_path, monkeypatch):
        config = _make_config(tmp_path)
        remote_cfg = RemoteExecutionConfig(
            provisioner=DispatchProvisionerConfig(
                type="gcp",
                settings={
                    "project_id": "my-project",
                    "zone": "us-central1-a",
                    "default_machine_type": "e2-standard-4",
                    "image_family": "ubuntu-2404-lts-amd64",
                },
            ),
            repo_url="https://github.com/test/test.git",
        )

        monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA_test_key")
        fake_mod = SimpleNamespace(
            InstancesClient=Mock(return_value=Mock()),
            ZoneOperationsClient=Mock(return_value=Mock()),
            MachineTypesClient=Mock(return_value=Mock()),
        )
        monkeypatch.setattr("tanren_core.adapters.gcp_vm._import_compute", lambda: fake_mod)

        env, _ = build_ssh_execution_environment(config, remote_cfg)

        assert isinstance(env, SSHExecutionEnvironment)
        assert env._provider == VMProvider.GCP


class TestValidateProvisionerRequirements:
    def test_manual_requires_nothing(self):
        validate_provisioner_requirements(ProvisionerType.MANUAL)

    def test_hetzner_requires_token(self, monkeypatch):
        monkeypatch.delenv("HCLOUD_TOKEN", raising=False)
        with pytest.raises(ValueError, match="HCLOUD_TOKEN"):
            validate_provisioner_requirements(ProvisionerType.HETZNER)

    def test_hetzner_passes_when_configured(self, monkeypatch):
        monkeypatch.setenv("HCLOUD_TOKEN", "test-token")
        # hcloud package is available in dev env; should pass with token set
        validate_provisioner_requirements(ProvisionerType.HETZNER)

    def test_gcp_requires_ssh_key(self, monkeypatch):
        monkeypatch.delenv("GCP_SSH_PUBLIC_KEY", raising=False)
        with pytest.raises(ValueError, match="GCP_SSH_PUBLIC_KEY"):
            validate_provisioner_requirements(ProvisionerType.GCP)

    def test_gcp_passes_when_configured(self, monkeypatch):
        monkeypatch.setenv("GCP_SSH_PUBLIC_KEY", "ssh-ed25519 AAAA_test")
        # google-cloud-compute is available in dev env; should pass with key set
        validate_provisioner_requirements(ProvisionerType.GCP)

    def test_error_message_includes_adapter_name(self, monkeypatch):
        monkeypatch.delenv("GCP_SSH_PUBLIC_KEY", raising=False)
        with pytest.raises(ValueError, match="Adapter 'gcp' configuration errors"):
            validate_provisioner_requirements(ProvisionerType.GCP)


class TestBuildExecutionEnvironment:
    def test_local_profile_builds_local_env(self, tmp_path):
        from tanren_core.adapters.local_environment import LocalExecutionEnvironment

        config = _make_config(tmp_path)
        profile = EnvironmentProfile(name="dev", type=EnvironmentProfileType.LOCAL)
        env, vm_store = build_execution_environment(config, profile)

        assert isinstance(env, LocalExecutionEnvironment)
        assert vm_store is None

    def test_remote_profile_without_config_raises(self, tmp_path):
        config = _make_config(tmp_path)
        profile = EnvironmentProfile(name="prod", type=EnvironmentProfileType.REMOTE)
        with pytest.raises(ValueError, match="remote_config is required"):
            build_execution_environment(config, profile)

    def test_remote_profile_with_config_builds_ssh_env(self, tmp_path):
        config = _make_config(tmp_path)
        remote_cfg = _manual_remote_cfg()
        profile = EnvironmentProfile(
            name="prod",
            type=EnvironmentProfileType.REMOTE,
            remote_config=remote_cfg,
        )
        env, vm_store = build_execution_environment(config, profile)

        assert isinstance(env, SSHExecutionEnvironment)
        assert vm_store is not None

    def test_docker_profile_raises_without_docker_config(self, tmp_path):
        config = _make_config(tmp_path)
        profile = EnvironmentProfile(name="ci", type=EnvironmentProfileType.DOCKER)
        with pytest.raises(ValueError, match="docker_config is required"):
            build_execution_environment(config, profile)

    def test_docker_profile_builds_docker_environment(self, tmp_path):
        from tanren_core.adapters.docker_environment import DockerExecutionEnvironment

        config = _make_config(tmp_path)
        docker_cfg = DockerExecutionConfig(
            repo_url="https://github.com/test/repo.git",
            required_clis=("claude",),
        )
        profile = EnvironmentProfile(
            name="ci-docker",
            type=EnvironmentProfileType.DOCKER,
            docker_config=docker_cfg,
        )
        env, vm_store = build_execution_environment(config, profile)
        assert isinstance(env, DockerExecutionEnvironment)
        assert vm_store is not None
