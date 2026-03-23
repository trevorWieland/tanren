"""Tests for markdown docs link and anchor validation."""

from typing import TYPE_CHECKING

from tanren_core.docs_links import (
    _find_repo_root,
    discover_markdown_files,
    validate_markdown_files,
)

if TYPE_CHECKING:
    from pathlib import Path


def _write(path: Path, content: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return path


class TestValidateMarkdownFiles:
    def test_passes_for_existing_relative_file_and_anchor(self, tmp_path: Path):
        repo = tmp_path
        source = _write(
            repo / "docs" / "map.md",
            "[overview](architecture/overview.md#what-tanren-is)\n",
        )
        _write(
            repo / "docs" / "architecture" / "overview.md",
            "# What Tanren Is\n\nBody\n",
        )

        errors = validate_markdown_files([source], repo)

        assert errors == []

    def test_detects_missing_target_file(self, tmp_path: Path):
        repo = tmp_path
        source = _write(repo / "docs" / "map.md", "[missing](architecture/missing.md)\n")

        errors = validate_markdown_files([source], repo)

        assert len(errors) == 1
        assert "target path not found" in errors[0].message
        assert errors[0].target == "architecture/missing.md"

    def test_detects_missing_anchor(self, tmp_path: Path):
        repo = tmp_path
        source = _write(repo / "docs" / "map.md", "[bad](overview.md#not-here)\n")
        _write(repo / "docs" / "overview.md", "# Existing Section\n")

        errors = validate_markdown_files([source], repo)

        assert len(errors) == 1
        assert "anchor not found" in errors[0].message
        assert errors[0].target == "overview.md#not-here"

    def test_supports_numbered_github_slug(self, tmp_path: Path):
        repo = tmp_path
        source = _write(
            repo / "docs" / "map.md", "[step](bootstrap.md#2-one-time-knowledge-bootstrap)\n"
        )
        _write(repo / "docs" / "bootstrap.md", "## 2. One-Time Knowledge Bootstrap\n")

        errors = validate_markdown_files([source], repo)

        assert errors == []

    def test_supports_duplicate_heading_suffix(self, tmp_path: Path):
        repo = tmp_path
        source = _write(repo / "docs" / "map.md", "[second](overview.md#repeated-1)\n")
        _write(repo / "docs" / "overview.md", "## Repeated\n\n## Repeated\n")

        errors = validate_markdown_files([source], repo)

        assert errors == []

    def test_ignores_links_in_fenced_code_blocks(self, tmp_path: Path):
        repo = tmp_path
        source = _write(
            repo / "docs" / "map.md",
            "```md\n[example](missing.md)\n```\n",
        )

        errors = validate_markdown_files([source], repo)

        assert errors == []

    def test_rejects_absolute_target_paths(self, tmp_path: Path):
        repo = tmp_path
        source = _write(repo / "docs" / "map.md", "[absolute](/tmp/outside.md)\n")

        errors = validate_markdown_files([source], repo)

        assert len(errors) == 1
        assert errors[0].message == "absolute target paths are not allowed"

    def test_rejects_paths_that_escape_repo_root(self, tmp_path: Path):
        root = tmp_path
        repo = root / "repo"
        source = _write(repo / "docs" / "map.md", "[outside](../../outside.md)\n")
        _write(root / "outside.md", "# Outside\n")

        errors = validate_markdown_files([source], repo)

        assert len(errors) == 1
        assert errors[0].message == "target path escapes repository root"

    def test_does_not_crash_for_escaped_markdown_anchor(self, tmp_path: Path):
        root = tmp_path
        repo = root / "repo"
        source = _write(repo / "docs" / "map.md", "[outside](../../outside.md#missing)\n")
        _write(root / "outside.md", "# Outside\n")

        errors = validate_markdown_files([source], repo)

        assert len(errors) == 1
        assert errors[0].message == "target path escapes repository root"


class TestFindRepoRoot:
    def test_finds_real_repo_root(self):
        """_find_repo_root returns a path containing a .git entry."""
        root = _find_repo_root()
        assert (root / ".git").exists()

    def test_accepts_git_directory(self, tmp_path: Path):
        """A .git directory (normal repo) is recognized."""
        repo = tmp_path / "repo"
        (repo / ".git").mkdir(parents=True)
        # .exists() matches directories
        assert (repo / ".git").exists()

    def test_accepts_git_file(self, tmp_path: Path):
        """A .git file (git worktree) is recognized by .exists()."""
        repo = tmp_path / "worktree"
        repo.mkdir()
        (repo / ".git").write_text("gitdir: /some/path/.git/worktrees/wt\n")
        # .exists() matches files — this is the worktree case
        assert (repo / ".git").exists()
        assert not (repo / ".git").is_dir()


class TestDiscoverMarkdownFiles:
    def test_discovers_repo_markdown_files_including_protocol_spec(self, tmp_path: Path):
        repo = tmp_path
        _write(repo / "README.md", "# Root\n")
        _write(repo / "docs" / "guide.md", "# Guide\n")
        _write(repo / "protocol" / "README.md", "# Protocol\n")
        _write(repo / ".venv" / "docs.md", "# Ignored\n")

        files = {path.relative_to(repo).as_posix() for path in discover_markdown_files(repo)}

        assert "README.md" in files
        assert "docs/guide.md" in files
        assert "protocol/README.md" in files
        assert ".venv/docs.md" not in files
