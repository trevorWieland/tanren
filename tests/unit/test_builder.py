"""Tests for the shared SSH execution environment builder."""

from __future__ import annotations

from pathlib import Path

import pytest
from pydantic import ValidationError

from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment
from tanren_core.builder import build_ssh_execution_environment
from tanren_core.config import Config


def _make_config(tmp_path: Path, remote_config_path: str) -> Config:
    return Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        remote_config_path=remote_config_path,
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
