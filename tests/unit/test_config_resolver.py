"""Tests for ConfigResolver implementations."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from pathlib import Path
import yaml

from tanren_core.config_resolver import DiskConfigResolver, GitHubConfigResolver


class TestDiskConfigResolver:
    @pytest.fixture
    def resolver(self, tmp_path: Path) -> DiskConfigResolver:
        return DiskConfigResolver(str(tmp_path))

    async def test_load_tanren_config_exists(self, resolver, tmp_path: Path) -> None:
        project_dir = tmp_path / "my-project"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text(
            yaml.dump({"environment": {"default": {"type": "local"}}})
        )
        result = await resolver.load_tanren_config("my-project")
        assert result["environment"]["default"]["type"] == "local"

    async def test_load_tanren_config_missing(self, resolver) -> None:
        result = await resolver.load_tanren_config("nonexistent")
        assert result == {}

    async def test_load_tanren_config_empty(self, resolver, tmp_path: Path) -> None:
        project_dir = tmp_path / "empty-project"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text("")
        result = await resolver.load_tanren_config("empty-project")
        assert result == {}

    async def test_load_tanren_config_ignores_branch(self, resolver, tmp_path: Path) -> None:
        project_dir = tmp_path / "proj"
        project_dir.mkdir()
        (project_dir / "tanren.yml").write_text(yaml.dump({"key": "value"}))
        # Branch parameter should be ignored for disk resolver
        result = await resolver.load_tanren_config("proj", branch="feature/xyz")
        assert result == {"key": "value"}

    async def test_load_project_env_exists(self, resolver, tmp_path: Path) -> None:
        project_dir = tmp_path / "proj"
        project_dir.mkdir()
        (project_dir / ".env").write_text("FOO=bar\nBAZ=qux\n")
        result = await resolver.load_project_env("proj")
        assert result == {"FOO": "bar", "BAZ": "qux"}

    async def test_load_project_env_missing(self, resolver) -> None:
        result = await resolver.load_project_env("nonexistent")
        assert result == {}


class TestGitHubConfigResolver:
    def test_parse_owner_repo_https(self) -> None:
        resolver = GitHubConfigResolver(repo_url_for=lambda _: None)
        assert resolver._parse_owner_repo("https://github.com/acme/widgets.git") == (
            "acme",
            "widgets",
        )

    def test_parse_owner_repo_ssh(self) -> None:
        resolver = GitHubConfigResolver(repo_url_for=lambda _: None)
        assert resolver._parse_owner_repo("git@github.com:acme/widgets.git") == ("acme", "widgets")

    def test_parse_owner_repo_invalid(self) -> None:
        resolver = GitHubConfigResolver(repo_url_for=lambda _: None)
        assert resolver._parse_owner_repo("https://gitlab.com/foo/bar") is None

    async def test_load_tanren_config_no_repo_url(self) -> None:
        resolver = GitHubConfigResolver(repo_url_for=lambda _: None)
        result = await resolver.load_tanren_config("unknown-project")
        assert result == {}

    async def test_load_project_env_no_repo_url(self) -> None:
        resolver = GitHubConfigResolver(repo_url_for=lambda _: None)
        result = await resolver.load_project_env("unknown-project")
        assert result == {}
