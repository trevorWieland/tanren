"""Integration tests for remote_config loading from real YAML files."""

from pathlib import Path

import pytest
from pydantic import ValidationError

from tanren_core.remote_config import (
    ExecutionMode,
    GitAuthMethod,
    ProvisionerType,
    load_remote_config,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _write_yaml(tmp_path: Path, content: str) -> Path:
    cfg = tmp_path / "remote.yml"
    cfg.write_text(content)
    return cfg


# ---------------------------------------------------------------------------
# Full / minimal loading
# ---------------------------------------------------------------------------


class TestLoadFullConfig:
    def test_load_full_config(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
execution_mode: remote
ssh:
  user: deploy
  key_path: ~/.ssh/id_rsa
  port: 22
git:
  auth: token
  token_env: GIT_TOKEN
provisioner:
  type: hetzner
  settings:
    token_env: HCLOUD_TOKEN
    default_server_type: cpx31
    location: ash
    image: ubuntu-24.04
    ssh_key_name: default
bootstrap:
  extra_script: setup.sh
secrets:
  developer_secrets_path: /path/to/secrets
repos:
  - project: myapp
    repo_url: https://github.com/org/myapp.git
""",
        )

        config = load_remote_config(path)

        assert config.execution_mode == ExecutionMode.REMOTE
        assert config.ssh.user == "deploy"
        assert config.ssh.key_path == "~/.ssh/id_rsa"
        assert config.ssh.port == 22
        assert config.git.auth == GitAuthMethod.TOKEN
        assert config.git.token_env == "GIT_TOKEN"
        assert config.provisioner.type == ProvisionerType.HETZNER
        assert config.provisioner.settings["token_env"] == "HCLOUD_TOKEN"
        assert config.provisioner.settings["default_server_type"] == "cpx31"
        assert config.provisioner.settings["location"] == "ash"
        assert config.provisioner.settings["image"] == "ubuntu-24.04"
        assert config.provisioner.settings["ssh_key_name"] == "default"
        assert config.bootstrap.extra_script == "setup.sh"
        assert config.secrets.developer_secrets_path == "/path/to/secrets"
        assert len(config.repos) == 1
        assert config.repos[0].project == "myapp"
        assert config.repos[0].repo_url == "https://github.com/org/myapp.git"


class TestLoadMinimalConfig:
    def test_load_minimal_config(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: manual
""",
        )

        config = load_remote_config(path)

        # Defaults
        assert config.execution_mode == ExecutionMode.REMOTE
        assert config.ssh.user == "root"
        assert config.ssh.key_path == "~/.ssh/tanren_vm"
        assert config.ssh.port == 22
        assert config.git.auth == GitAuthMethod.TOKEN
        assert config.git.token_env == "GIT_TOKEN"
        assert config.provisioner.type == ProvisionerType.MANUAL
        assert config.provisioner.settings == {}
        assert config.bootstrap.extra_script is None
        assert not config.secrets.developer_secrets_path
        assert config.repos == []


# ---------------------------------------------------------------------------
# File not found
# ---------------------------------------------------------------------------


class TestLoadFileNotFound:
    def test_load_file_not_found(self, tmp_path: Path) -> None:
        with pytest.raises(FileNotFoundError):
            load_remote_config(tmp_path / "nope.yml")


# ---------------------------------------------------------------------------
# Repos coercion
# ---------------------------------------------------------------------------


class TestReposAsDictShorthand:
    def test_repos_as_dict_shorthand(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: manual
repos:
  myapp: https://github.com/org/myapp.git
  other: https://github.com/org/other.git
""",
        )

        config = load_remote_config(path)

        assert len(config.repos) == 2
        projects = {r.project: r.repo_url for r in config.repos}
        assert projects["myapp"] == "https://github.com/org/myapp.git"
        assert projects["other"] == "https://github.com/org/other.git"


class TestReposAsListWithMetadata:
    def test_repos_as_list_with_metadata(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: manual
repos:
  - project: myapp
    repo_url: https://github.com/org/myapp.git
    metadata:
      team: backend
""",
        )

        config = load_remote_config(path)

        assert len(config.repos) == 1
        assert config.repos[0].project == "myapp"
        assert config.repos[0].repo_url == "https://github.com/org/myapp.git"
        assert config.repos[0].metadata == {"team": "backend"}


class TestReposEmpty:
    def test_repos_empty(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: manual
""",
        )

        config = load_remote_config(path)

        assert config.repos == []


# ---------------------------------------------------------------------------
# repo_url_for
# ---------------------------------------------------------------------------


class TestRepoUrlForExisting:
    def test_repo_url_for_existing(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: manual
repos:
  - project: myapp
    repo_url: https://github.com/org/myapp.git
""",
        )

        config = load_remote_config(path)

        assert config.repo_url_for("myapp") == "https://github.com/org/myapp.git"


class TestRepoUrlForMissing:
    def test_repo_url_for_missing(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: manual
repos:
  - project: myapp
    repo_url: https://github.com/org/myapp.git
""",
        )

        config = load_remote_config(path)

        assert config.repo_url_for("nonexistent") is None


# ---------------------------------------------------------------------------
# Provisioner settings
# ---------------------------------------------------------------------------


class TestProvisionerWithSettings:
    def test_provisioner_with_settings(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: hetzner
  settings:
    token_env: HCLOUD_TOKEN
    default_server_type: cpx31
    location: ash
""",
        )

        config = load_remote_config(path)

        assert config.provisioner.type == ProvisionerType.HETZNER
        assert config.provisioner.settings["token_env"] == "HCLOUD_TOKEN"
        assert config.provisioner.settings["default_server_type"] == "cpx31"
        assert config.provisioner.settings["location"] == "ash"


class TestProvisionerWithoutSettings:
    def test_provisioner_without_settings(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: manual
""",
        )

        config = load_remote_config(path)

        assert config.provisioner.type == ProvisionerType.MANUAL
        assert config.provisioner.settings == {}


# ---------------------------------------------------------------------------
# Edge cases
# ---------------------------------------------------------------------------


class TestEmptyYamlFile:
    def test_empty_yaml_file(self, tmp_path: Path) -> None:
        path = _write_yaml(tmp_path, "")

        with pytest.raises(ValidationError):
            load_remote_config(path)


class TestHetznerProvisionerSettings:
    def test_hetzner_provisioner_settings(self, tmp_path: Path) -> None:
        path = _write_yaml(
            tmp_path,
            """\
provisioner:
  type: hetzner
  settings:
    token_env: HCLOUD_TOKEN
    default_server_type: cpx31
    location: ash
    image: ubuntu-24.04
    ssh_key_name: default
""",
        )

        config = load_remote_config(path)

        assert config.provisioner.type == ProvisionerType.HETZNER
        settings = config.provisioner.settings
        assert settings["token_env"] == "HCLOUD_TOKEN"
        assert settings["default_server_type"] == "cpx31"
        assert settings["location"] == "ash"
        assert settings["image"] == "ubuntu-24.04"
        assert settings["ssh_key_name"] == "default"
