"""Tests for core StrEnum types."""

from worker_manager.env.environment_schema import EnvironmentProfileType
from worker_manager.remote_config import ExecutionMode, GitAuthMethod, ProvisionerType
from worker_manager.roles import AuthMode


def test_execution_mode_enum_values() -> None:
    assert ExecutionMode.REMOTE.value == "remote"
    assert ExecutionMode.LOCAL.value == "local"


def test_provisioner_type_enum_values() -> None:
    assert ProvisionerType.MANUAL.value == "manual"
    assert ProvisionerType.HETZNER.value == "hetzner"


def test_git_auth_method_enum_values() -> None:
    assert GitAuthMethod.TOKEN.value == "token"
    assert GitAuthMethod.SSH.value == "ssh"


def test_cli_auth_method_enum_values() -> None:
    assert AuthMode.API_KEY.value == "api_key"
    assert AuthMode.OAUTH.value == "oauth"


def test_environment_profile_type_enum_values() -> None:
    assert EnvironmentProfileType.LOCAL.value == "local"
    assert EnvironmentProfileType.REMOTE.value == "remote"
    assert EnvironmentProfileType.DOCKER.value == "docker"
