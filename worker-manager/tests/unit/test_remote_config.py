"""Tests for remote config loader."""

from pathlib import Path

import pytest

from worker_manager.remote_config import (
    RemoteConfig,
    RemoteGitConfig,
    RemoteRepoBinding,
    RemoteSSHConfig,
    RemoteVMConfig,
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
vms:
  - id: vm-1
    host: 10.0.0.1
  - id: vm-2
    host: 10.0.0.2
bootstrap:
  extra_script: setup.sh
secrets:
  developer_secrets_path: /tmp/secrets
repos:
  myproject: https://github.com/org/myproject.git
"""

MINIMAL_YAML = """\
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
        assert len(cfg.vms) == 2
        assert cfg.vms[0].host == "10.0.0.1"
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
        assert len(cfg.vms) == 1
        assert isinstance(cfg.vms[0], RemoteVMConfig)
        assert cfg.bootstrap.extra_script is None
        assert cfg.secrets.developer_secrets_path == ""
        assert cfg.repos == []

    def test_missing_file_raises(self, tmp_path: Path):
        missing = tmp_path / "nonexistent.yml"
        with pytest.raises(FileNotFoundError, match="Remote config not found"):
            load_remote_config(missing)


class TestRemoteConfigDefaults:
    def test_remote_config_defaults(self):
        cfg = RemoteConfig()

        assert cfg.execution_mode == "remote"
        assert isinstance(cfg.ssh, RemoteSSHConfig)
        assert isinstance(cfg.git, RemoteGitConfig)
        assert cfg.vms == []
        assert cfg.bootstrap.extra_script is None
        assert cfg.secrets.developer_secrets_path == ""
        assert cfg.repos == []


class TestSSHAndGitDefaults:
    def test_ssh_defaults(self):
        ssh = RemoteSSHConfig()
        assert ssh.user == "root"
        assert ssh.key_path == "~/.ssh/tanren_vm"
        assert ssh.port == 22
        assert ssh.connect_timeout == 10

    def test_git_defaults(self):
        git = RemoteGitConfig()
        assert git.auth == "token"
        assert git.token_env == "GIT_TOKEN"


class TestRepoBindings:
    def test_repos_map_coerces_to_bindings(self, tmp_path: Path):
        cfg_file = tmp_path / "remote.yml"
        cfg_file.write_text("repos:\n  api: https://example.com/repo.git\n")
        cfg = load_remote_config(cfg_file)
        assert cfg.repos == [
            RemoteRepoBinding(project="api", repo_url="https://example.com/repo.git")
        ]
