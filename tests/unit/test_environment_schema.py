"""Tests for environment_schema module."""

import pytest

from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    EnvironmentProfileType,
    McpServerConfig,
    ResourceRequirements,
    parse_environment_profiles,
)


class TestParseEnvironmentProfilesEmptyDict:
    def test_empty_dict_returns_default(self):
        result = parse_environment_profiles({})
        assert "default" in result
        assert len(result) == 1
        assert result["default"].name == "default"
        assert result["default"].type == EnvironmentProfileType.LOCAL


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
        with pytest.raises(ValueError):
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
        # "default" key is absent from input, but the function always adds
        # one if missing
        assert "default" in result
        assert len(result) == 3

        assert result["local"].gate_cmd == "pytest"
        assert result["staging"].resources.cpu == 8
        assert result["staging"].resources.memory_gb == 16
        assert result["staging"].setup == ("docker compose up -d",)


class TestMcpServerConfig:
    def test_url_required(self):
        with pytest.raises(ValueError):
            McpServerConfig()

    def test_headers_default_empty(self):
        cfg = McpServerConfig(url="https://example.com/sse")
        assert cfg.headers == {}

    def test_extra_fields_forbidden(self):
        with pytest.raises(ValueError):
            McpServerConfig(url="https://example.com/sse", bogus="x")  # type: ignore[unknown-argument]

    def test_frozen(self):
        cfg = McpServerConfig(url="https://example.com/sse")
        with pytest.raises(ValueError):
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
