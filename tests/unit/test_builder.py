"""Tests for the shared SSH execution environment builder."""

from __future__ import annotations

from pathlib import Path

import pytest
from pydantic import ValidationError

from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.remote_types import VMProvider
from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment
from tanren_core.builder import build_ssh_execution_environment
from tanren_core.config import Config

_ROLES_YML = """\
agents:
  default:
    cli: claude
    auth: subscription
    model: opus
"""


def _make_config(tmp_path: Path, remote_config_path: str | None) -> Config:
    roles_path = tmp_path / "roles.yml"
    roles_path.write_text(_ROLES_YML)
    return Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        remote_config_path=remote_config_path,
        roles_config_path=str(roles_path),
    )


class TestBuildSSHExecutionEnvironment:
    def test_build_manual_provisioner(self, tmp_path):
        remote_yml = tmp_path / "remote.yml"
        remote_yml.write_text("""\
provisioner:
  type: manual
  settings:
    vms:
      - id: vm-1
        host: "10.0.0.1"
repos:
  - project: test
    repo_url: https://github.com/test/test.git
""")
        config = _make_config(tmp_path, str(remote_yml))
        emitter = NullEventEmitter()

        env, state_store = build_ssh_execution_environment(config, emitter)

        assert isinstance(env, SSHExecutionEnvironment)
        assert state_store is not None

    def test_build_manual_provisioner_sets_provider(self, tmp_path):
        remote_yml = tmp_path / "remote.yml"
        remote_yml.write_text("""\
provisioner:
  type: manual
  settings:
    vms:
      - id: vm-1
        host: "10.0.0.1"
repos:
  - project: test
    repo_url: https://github.com/test/test.git
""")
        config = _make_config(tmp_path, str(remote_yml))
        emitter = NullEventEmitter()

        env, _ = build_ssh_execution_environment(config, emitter)

        assert env._provider == VMProvider.MANUAL

    def test_build_unsupported_provisioner_raises(self, tmp_path):
        remote_yml = tmp_path / "remote.yml"
        remote_yml.write_text("""\
provisioner:
  type: aws
  settings: {}
repos: []
""")
        config = _make_config(tmp_path, str(remote_yml))
        emitter = NullEventEmitter()

        with pytest.raises((ValueError, ValidationError)):
            build_ssh_execution_environment(config, emitter)

    def test_build_no_remote_config_path_raises_value_error(self, tmp_path):
        config = _make_config(tmp_path, remote_config_path=None)
        emitter = NullEventEmitter()

        with pytest.raises(ValueError, match="remote_config_path is required"):
            build_ssh_execution_environment(config, emitter)

    def test_build_uses_roles_for_credential_providers(self, tmp_path):
        """Builder uses roles.yml to determine credential providers."""
        roles_path = tmp_path / "roles.yml"
        roles_path.write_text("""\
agents:
  default:
    cli: claude
    auth: subscription
    model: opus
  audit:
    cli: codex
    auth: subscription
    model: o3
""")
        remote_yml = tmp_path / "remote.yml"
        remote_yml.write_text("""\
provisioner:
  type: manual
  settings:
    vms:
      - id: vm-1
        host: "10.0.0.1"
repos:
  - project: test
    repo_url: https://github.com/test/test.git
""")
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            remote_config_path=str(remote_yml),
            roles_config_path=str(roles_path),
        )
        emitter = NullEventEmitter()

        env, _ = build_ssh_execution_environment(config, emitter)

        provider_names = sorted(p.name for p in env._credential_providers)
        assert provider_names == ["claude", "codex"]

    def test_build_sets_agent_user(self, tmp_path):
        """Builder configures agent_user on SSHExecutionEnvironment."""
        remote_yml = tmp_path / "remote.yml"
        remote_yml.write_text("""\
provisioner:
  type: manual
  settings:
    vms:
      - id: vm-1
        host: "10.0.0.1"
repos:
  - project: test
    repo_url: https://github.com/test/test.git
""")
        config = _make_config(tmp_path, str(remote_yml))
        emitter = NullEventEmitter()

        env, _ = build_ssh_execution_environment(config, emitter)

        assert env._agent_user == "tanren"
