"""Tests for environment_schema module."""

import pytest
from pydantic import ValidationError

from tanren_core.env.environment_schema import (
    DispatchProvisionerConfig,
    DockerExecutionConfig,
    EnvironmentProfile,
    EnvironmentProfileType,
    IssueSourceConfig,
    IssueSourceType,
    McpServerConfig,
    RemoteExecutionConfig,
    ResourceRequirements,
    parse_environment_profiles,
    parse_issue_source,
)


class TestParseEnvironmentProfilesEmptyDict:
    def test_empty_dict_returns_empty(self):
        result = parse_environment_profiles({})
        assert len(result) == 0


class TestParseEnvironmentProfilesFullConfig:
    def test_full_config_parses_correctly(self):
        data = {
            "environment": {
                "ci": {
                    "type": "docker",
                    "server_type": "cpx31",
                    "resources": {"cpu": 4, "memory_gb": 8, "gpu": True},
                    "setup": ["pip install -e .", "make build"],
                    "teardown": ["make clean"],
                    "gate_cmd": "make test",
                }
            }
        }
        result = parse_environment_profiles(data)
        ci = result["ci"]
        assert ci.name == "ci"
        assert ci.type == EnvironmentProfileType.DOCKER
        assert ci.server_type == "cpx31"
        assert ci.resources.cpu == 4
        assert ci.resources.memory_gb == 8
        assert ci.resources.gpu is True
        assert ci.setup == ("pip install -e .", "make build")
        assert ci.teardown == ("make clean",)
        assert ci.gate_cmd == "make test"


class TestResourceRequirementsDefaults:
    def test_defaults(self):
        r = ResourceRequirements()
        assert r.cpu == 2
        assert r.memory_gb == 4
        assert r.gpu is False


class TestEnvironmentProfileDefaults:
    def test_defaults(self):
        p = EnvironmentProfile(name="test")
        assert p.type == EnvironmentProfileType.LOCAL
        assert p.setup == ()
        assert p.teardown == ()
        assert p.gate_cmd == "make check"
        assert p.server_type is None
        assert p.resources == ResourceRequirements()


class TestInvalidTypeRaised:
    def test_arbitrary_type_raises(self):
        data = {"environment": {"bad": {"type": 12345}}}
        with pytest.raises(ValueError, match="Input should be"):
            parse_environment_profiles(data)


class TestMultipleProfiles:
    def test_multiple_profiles_parsed(self):
        data = {
            "environment": {
                "local": {"type": "local", "gate_cmd": "pytest"},
                "staging": {
                    "type": "docker",
                    "resources": {"cpu": 8, "memory_gb": 16},
                    "setup": ["docker compose up -d"],
                    "teardown": ["docker compose down"],
                    "gate_cmd": "make integration",
                },
            }
        }
        result = parse_environment_profiles(data)
        assert "local" in result
        assert "staging" in result
        assert "default" not in result
        assert len(result) == 2

        assert result["local"].gate_cmd == "pytest"
        assert result["staging"].resources.cpu == 8
        assert result["staging"].resources.memory_gb == 16
        assert result["staging"].setup == ("docker compose up -d",)


class TestEnvironmentProfileGateFields:
    def test_task_gate_cmd_default_none(self):
        p = EnvironmentProfile(name="test")
        assert p.task_gate_cmd is None

    def test_spec_gate_cmd_default_none(self):
        p = EnvironmentProfile(name="test")
        assert p.spec_gate_cmd is None

    def test_task_gate_cmd_set(self):
        p = EnvironmentProfile(name="test", task_gate_cmd="make unit")
        assert p.task_gate_cmd == "make unit"
        assert p.gate_cmd == "make check"

    def test_spec_gate_cmd_set(self):
        p = EnvironmentProfile(name="test", spec_gate_cmd="make integration")
        assert p.spec_gate_cmd == "make integration"
        assert p.gate_cmd == "make check"

    def test_both_set_with_gate_cmd(self):
        p = EnvironmentProfile(
            name="test",
            gate_cmd="make all",
            task_gate_cmd="make unit",
            spec_gate_cmd="make integration",
        )
        assert p.gate_cmd == "make all"
        assert p.task_gate_cmd == "make unit"
        assert p.spec_gate_cmd == "make integration"

    def test_parsed_from_yml(self):
        data = {
            "environment": {
                "ci": {
                    "gate_cmd": "make check",
                    "task_gate_cmd": "make unit",
                    "spec_gate_cmd": "make integration",
                }
            }
        }
        result = parse_environment_profiles(data)
        ci = result["ci"]
        assert ci.task_gate_cmd == "make unit"
        assert ci.spec_gate_cmd == "make integration"

    def test_frozen(self):
        p = EnvironmentProfile(name="test", task_gate_cmd="make unit")
        with pytest.raises(ValueError, match="Instance is frozen"):
            p.task_gate_cmd = "changed"


class TestMcpServerConfig:
    def test_url_required(self):
        with pytest.raises(ValueError, match="url"):
            McpServerConfig()

    def test_headers_default_empty(self):
        cfg = McpServerConfig(url="https://example.com/sse")
        assert cfg.headers == {}

    def test_extra_fields_forbidden(self):
        with pytest.raises(ValueError, match="Extra inputs are not permitted"):
            McpServerConfig.model_validate({"url": "https://example.com/sse", "bogus": "x"})

    def test_frozen(self):
        cfg = McpServerConfig(url="https://example.com/sse")
        with pytest.raises(ValueError, match="Instance is frozen"):
            cfg.url = "changed"


class TestEnvironmentProfileMcp:
    def test_mcp_default_empty(self):
        p = EnvironmentProfile(name="test")
        assert p.mcp == {}

    def test_mcp_parsed_from_dict(self):
        data = {
            "environment": {
                "default": {
                    "type": "remote",
                    "mcp": {
                        "context7": {
                            "url": "https://mcp.context7.com/sse",
                            "headers": {"Authorization": "MCP_CONTEXT7_KEY"},
                        }
                    },
                }
            }
        }
        result = parse_environment_profiles(data)
        profile = result["default"]
        assert "context7" in profile.mcp
        assert profile.mcp["context7"].url == "https://mcp.context7.com/sse"
        assert profile.mcp["context7"].headers == {"Authorization": "MCP_CONTEXT7_KEY"}

    def test_mcp_multiple_servers(self):
        data = {
            "environment": {
                "default": {
                    "type": "remote",
                    "mcp": {
                        "ctx7": {"url": "https://ctx7.example.com/sse"},
                        "other": {
                            "url": "https://other.example.com/sse",
                            "headers": {"X-Api-Key": "OTHER_KEY"},
                        },
                    },
                }
            }
        }
        result = parse_environment_profiles(data)
        profile = result["default"]
        assert len(profile.mcp) == 2
        assert profile.mcp["ctx7"].headers == {}
        assert profile.mcp["other"].headers == {"X-Api-Key": "OTHER_KEY"}

    def test_mcp_server_name_with_dot_rejected(self):
        with pytest.raises(ValueError, match="must match"):
            EnvironmentProfile(
                name="test",
                mcp={"my.server": McpServerConfig(url="https://example.com/sse")},
            )

    def test_mcp_server_name_with_space_rejected(self):
        with pytest.raises(ValueError, match="must match"):
            EnvironmentProfile(
                name="test",
                mcp={"my server": McpServerConfig(url="https://example.com/sse")},
            )

    def test_mcp_server_name_with_hyphen_allowed(self):
        p = EnvironmentProfile(
            name="test",
            mcp={"my-server": McpServerConfig(url="https://example.com/sse")},
        )
        assert "my-server" in p.mcp


class TestIssueSourceType:
    def test_github_value(self):
        assert IssueSourceType.GITHUB == "github"

    def test_linear_value(self):
        assert IssueSourceType.LINEAR == "linear"


class TestIssueSourceConfig:
    def test_default_type_is_github(self):
        cfg = IssueSourceConfig()
        assert cfg.type == IssueSourceType.GITHUB
        assert cfg.settings == {}

    def test_explicit_linear_type(self):
        cfg = IssueSourceConfig(type=IssueSourceType.LINEAR, settings={"team": "ENG"})
        assert cfg.type == IssueSourceType.LINEAR
        assert cfg.settings == {"team": "ENG"}

    def test_frozen(self):
        cfg = IssueSourceConfig()
        with pytest.raises(ValueError, match="Instance is frozen"):
            cfg.type = IssueSourceType.LINEAR

    def test_extra_forbidden(self):
        with pytest.raises(ValueError, match="Extra inputs are not permitted"):
            IssueSourceConfig.model_validate({"type": "github", "bogus": "x"})


class TestParseIssueSource:
    def test_returns_none_when_absent(self):
        assert parse_issue_source({}) is None

    def test_parses_github_config(self):
        data = {
            "issue_source": {
                "type": "github",
                "settings": {"owner": "myorg", "repo": "myrepo"},
            }
        }
        cfg = parse_issue_source(data)
        assert cfg is not None
        assert cfg.type == IssueSourceType.GITHUB
        assert cfg.settings == {"owner": "myorg", "repo": "myrepo"}

    def test_returns_none_for_non_dict(self):
        assert parse_issue_source({"issue_source": "invalid"}) is None


class TestBootstrapExtraScriptUrl:
    """Tests for bootstrap_extra_script_url on execution configs."""

    def test_remote_config_url_default_none(self):
        cfg = RemoteExecutionConfig(
            provisioner=DispatchProvisionerConfig(type="manual"),
            repo_url="https://github.com/org/repo.git",
        )
        assert cfg.bootstrap_extra_script_url is None
        assert cfg.bootstrap_extra_script is None

    def test_remote_config_url_set(self):
        cfg = RemoteExecutionConfig(
            provisioner=DispatchProvisionerConfig(type="manual"),
            repo_url="https://github.com/org/repo.git",
            bootstrap_extra_script_url="https://example.com/setup.sh",
        )
        assert cfg.bootstrap_extra_script_url == "https://example.com/setup.sh"

    def test_remote_config_mutual_exclusivity(self):
        with pytest.raises(ValidationError, match="mutually exclusive"):
            RemoteExecutionConfig(
                provisioner=DispatchProvisionerConfig(type="manual"),
                repo_url="https://github.com/org/repo.git",
                bootstrap_extra_script="#!/bin/bash\necho hi",
                bootstrap_extra_script_url="https://example.com/setup.sh",
            )

    def test_docker_config_url_default_none(self):
        cfg = DockerExecutionConfig(repo_url="https://github.com/org/repo.git")
        assert cfg.bootstrap_extra_script_url is None

    def test_docker_config_url_set(self):
        cfg = DockerExecutionConfig(
            repo_url="https://github.com/org/repo.git",
            bootstrap_extra_script_url="gs://bucket/script.sh",
        )
        assert cfg.bootstrap_extra_script_url == "gs://bucket/script.sh"

    def test_docker_config_mutual_exclusivity(self):
        with pytest.raises(ValidationError, match="mutually exclusive"):
            DockerExecutionConfig(
                repo_url="https://github.com/org/repo.git",
                bootstrap_extra_script="#!/bin/bash\necho hi",
                bootstrap_extra_script_url="https://example.com/setup.sh",
            )
