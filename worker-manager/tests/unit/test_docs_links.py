"""Tests for markdown docs link and anchor validation."""

from pathlib import Path

from worker_manager.docs_links import validate_markdown_files


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
