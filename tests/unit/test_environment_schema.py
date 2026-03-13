"""Tests for environment_schema module."""

import pytest

from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    EnvironmentProfileType,
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
