"""Tests for remote config loader."""

from pathlib import Path

import pytest
from pydantic import ValidationError

from tanren_core.remote_config import (
    ProvisionerType,
    RemoteConfig,
    RemoteGitConfig,
    RemoteProvisionerConfig,
    RemoteRepoBinding,
    RemoteSSHConfig,
    load_remote_config,
)

FULL_YAML = """\
execution_mode: remote
ssh:
  user: deploy
  key_path: ~/.ssh/my_key
  connect_timeout: 30
git:
  auth: ssh
  token_env: MY_GIT_TOKEN
provisioner:
  type: manual
  settings:
    vms:
      - id: vm-1
        host: 10.0.0.1
      - vm_id: vm-2
        host: 10.0.0.2
bootstrap:
  extra_script: setup.sh
secrets:
  developer_secrets_path: /tmp/secrets
repos:
  myproject: https://github.com/org/myproject.git
"""

MINIMAL_YAML = """\
provisioner:
  type: manual
  settings:
    vms:
      - id: vm-1
        host: 10.0.0.1
"""


class TestLoadRemoteConfig:
    def test_full_yaml(self, tmp_path: Path):
        cfg_file = tmp_path / "remote.yml"
        cfg_file.write_text(FULL_YAML)

        cfg = load_remote_config(cfg_file)

        assert cfg.execution_mode == "remote"
        assert cfg.ssh.user == "deploy"
        assert cfg.ssh.key_path == "~/.ssh/my_key"
        assert cfg.ssh.connect_timeout == 30
        assert cfg.git.auth == "ssh"
        assert cfg.git.token_env == "MY_GIT_TOKEN"
        assert cfg.provisioner.type == ProvisionerType.MANUAL
        assert isinstance(cfg.provisioner.settings, dict)
        assert cfg.bootstrap.extra_script == "setup.sh"
        assert cfg.secrets.developer_secrets_path == "/tmp/secrets"
        assert cfg.repos[0].project == "myproject"
        assert cfg.repos[0].repo_url == "https://github.com/org/myproject.git"

    def test_minimal_yaml_uses_defaults(self, tmp_path: Path):
        cfg_file = tmp_path / "remote.yml"
        cfg_file.write_text(MINIMAL_YAML)

        cfg = load_remote_config(cfg_file)

        assert cfg.execution_mode == "remote"
        assert cfg.ssh.user == "root"
        assert cfg.ssh.key_path == "~/.ssh/tanren_vm"
        assert cfg.ssh.port == 22
        assert cfg.ssh.connect_timeout == 10
        assert cfg.git.auth == "token"
        assert cfg.git.token_env == "GIT_TOKEN"
        assert cfg.provisioner.type == ProvisionerType.MANUAL
        assert cfg.bootstrap.extra_script is None
        assert not cfg.secrets.developer_secrets_path
        assert cfg.repos == []

    def test_missing_file_raises(self, tmp_path: Path):
        missing = tmp_path / "nonexistent.yml"
        with pytest.raises(FileNotFoundError, match="Remote config not found"):
            load_remote_config(missing)

    def test_missing_provisioner_block_raises(self, tmp_path: Path):
        cfg_file = tmp_path / "remote.yml"
        cfg_file.write_text("ssh:\n  user: root\n")
        with pytest.raises(ValidationError):
            load_remote_config(cfg_file)

    def test_legacy_vms_only_schema_rejected(self, tmp_path: Path):
        cfg_file = tmp_path / "remote.yml"
        cfg_file.write_text("vms:\n  - id: vm-1\n    host: 10.0.0.1\n")
        with pytest.raises(ValidationError):
            load_remote_config(cfg_file)


class TestRemoteConfigDefaults:
    def test_remote_config_defaults(self):
        cfg = RemoteConfig(
            provisioner=RemoteProvisionerConfig(
                type=ProvisionerType.MANUAL,
                settings={},
            )
        )

        assert cfg.execution_mode == "remote"
        assert isinstance(cfg.ssh, RemoteSSHConfig)
        assert isinstance(cfg.git, RemoteGitConfig)
        assert cfg.bootstrap.extra_script is None
        assert not cfg.secrets.developer_secrets_path
        assert cfg.repos == []


class TestRepoBindings:
    def test_repos_map_coerces_to_bindings(self, tmp_path: Path):
        cfg_file = tmp_path / "remote.yml"
        cfg_file.write_text(
            "provisioner:\n"
            "  type: manual\n"
            "  settings: {}\n"
            "repos:\n"
            "  api: https://example.com/repo.git\n"
        )
        cfg = load_remote_config(cfg_file)
        assert cfg.repos == [
            RemoteRepoBinding(project="api", repo_url="https://example.com/repo.git")
        ]
