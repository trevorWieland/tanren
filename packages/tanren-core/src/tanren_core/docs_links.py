"""Validate local markdown links and heading anchors for canonical docs."""

from __future__ import annotations

import argparse
import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote

_LINK_RE = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]+)\)")
_HEADING_RE = re.compile(r"^\s{0,3}#{1,6}\s+(.+?)\s*$")
_CODE_FENCE_RE = re.compile(r"^\s*(```|~~~)")
_TITLE_SUFFIX_RE = re.compile(r'\s+"[^"]*"\s*$')
_WHITESPACE_RE = re.compile(r"[\s-]+")


def _find_repo_root() -> Path:
    """Walk up from this file to find the repository root.

    Checks for a ``.git`` entry that is either a directory (normal repo)
    or a file (git worktree).

    Returns:
        Path to the repository root directory.

    Raises:
        FileNotFoundError: If no .git entry is found in any parent.
    """
    current = Path(__file__).resolve().parent
    while current != current.parent:
        if (current / ".git").exists():
            return current
        current = current.parent
    raise FileNotFoundError("Could not find repository root (.git entry)")


_EXCLUDED_DIR_NAMES = {
    ".git",
    ".venv",
    "__pycache__",
    "node_modules",
    ".mypy_cache",
    ".ruff_cache",
    ".pytest_cache",
}


@dataclass(frozen=True)
class LinkError:
    """A broken or invalid link found during markdown validation."""

    source_file: Path
    target: str
    message: str


def _slugify_github_heading(heading_text: str) -> str:
    text = heading_text.strip().rstrip("#").strip().lower()
    text = re.sub(r"[^\w\s-]", "", text)
    text = _WHITESPACE_RE.sub("-", text).strip("-")
    return text


def _extract_anchors(markdown_file: Path) -> set[str]:
    anchors: set[str] = set()
    counts: Counter[str] = Counter()
    in_fence = False
    for line in markdown_file.read_text(encoding="utf-8").splitlines():
        if _CODE_FENCE_RE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        match = _HEADING_RE.match(line)
        if not match:
            continue
        base = _slugify_github_heading(match.group(1))
        if not base:
            continue
        index = counts[base]
        anchor = base if index == 0 else f"{base}-{index}"
        counts[base] += 1
        anchors.add(anchor)
    return anchors


def _iter_local_links(markdown_text: str) -> list[str]:
    links: list[str] = []
    in_fence = False
    for line in markdown_text.splitlines():
        if _CODE_FENCE_RE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        for raw_target in _LINK_RE.findall(line):
            target = raw_target.strip()
            target = _TITLE_SUFFIX_RE.sub("", target).strip()
            if target.startswith("<") and target.endswith(">"):
                target = target[1:-1].strip()
            target = unquote(target)
            links.append(target)
    return links


def _strip_query_and_fragment(target: str) -> tuple[str, str]:
    without_query = target.split("?", maxsplit=1)[0]
    if "#" in without_query:
        path_part, fragment = without_query.split("#", maxsplit=1)
        return path_part, fragment
    return without_query, ""


def _is_external_link(target: str) -> bool:
    lowered = target.lower()
    return lowered.startswith(("http://", "https://", "mailto:", "tel:"))


def _is_excluded_markdown(path: Path, repo_root: Path) -> bool:
    try:
        rel_parts = path.relative_to(repo_root).parts
    except ValueError:
        return True
    return any(part in _EXCLUDED_DIR_NAMES for part in rel_parts[:-1])


def _format_repo_relative(path: Path, repo_root: Path) -> str:
    try:
        return str(path.relative_to(repo_root))
    except ValueError:
        return str(path)


def discover_markdown_files(repo_root: Path) -> list[Path]:
    """Find all non-excluded markdown files under the repo root.

    Returns:
        Sorted list of resolved markdown file paths.
    """
    repo_root = repo_root.resolve()
    files: set[Path] = set()
    for path in repo_root.rglob("*.md"):
        if not path.is_file():
            continue
        resolved = path.resolve()
        if _is_excluded_markdown(resolved, repo_root):
            continue
        files.add(resolved)
    return sorted(files)


def validate_markdown_files(files: list[Path], repo_root: Path) -> list[LinkError]:
    """Validate all local links and anchors in the given markdown files.

    Returns:
        List of LinkError instances for broken links.
    """
    repo_root = repo_root.resolve()
    errors: list[LinkError] = []
    anchor_cache: dict[Path, set[str]] = {}

    def anchors_for(target_file: Path) -> set[str]:
        cached = anchor_cache.get(target_file)
        if cached is None:
            cached = _extract_anchors(target_file)
            anchor_cache[target_file] = cached
        return cached

    for source_file in files:
        links = _iter_local_links(source_file.read_text(encoding="utf-8"))
        for target in links:
            if not target or _is_external_link(target):
                continue
            path_part, fragment = _strip_query_and_fragment(target)

            if path_part:
                candidate = Path(path_part)
                if candidate.is_absolute():
                    errors.append(
                        LinkError(source_file, target, "absolute target paths are not allowed")
                    )
                    continue
                resolved = (source_file.parent / candidate).resolve()
            else:
                resolved = source_file.resolve()

            try:
                resolved.relative_to(repo_root)
            except ValueError:
                errors.append(LinkError(source_file, target, "target path escapes repository root"))
                continue

            if not resolved.exists():
                errors.append(LinkError(source_file, target, f"target path not found: {path_part}"))
                continue
            if not resolved.is_file():
                errors.append(LinkError(source_file, target, "target is not a file"))
                continue
            if (
                fragment
                and resolved.suffix.lower() == ".md"
                and fragment not in anchors_for(resolved)
            ):
                errors.append(
                    LinkError(
                        source_file,
                        target,
                        f"anchor not found in {_format_repo_relative(resolved, repo_root)}",
                    )
                )
    return errors


def validate_repo_docs(repo_root: Path) -> list[LinkError]:
    """Discover and validate all markdown files in the repository.

    Returns:
        List of LinkError instances for broken links.
    """
    files = discover_markdown_files(repo_root)
    return validate_markdown_files(files, repo_root)


def main() -> int:
    """CLI entry point for docs link validation.

    Returns:
        Exit code (0 for success, 1 for failures).
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=_find_repo_root(),
        help="Tanren repository root path",
    )
    args = parser.parse_args()
    repo_root = args.repo_root.resolve()

    errors = validate_repo_docs(repo_root)
    if not errors:
        print(f"Docs link check passed ({len(discover_markdown_files(repo_root))} files).")
        return 0

    for error in errors:
        rel_source = _format_repo_relative(error.source_file, repo_root)
        print(f"{rel_source}: {error.message} [{error.target}]")
    print(f"Docs link check failed with {len(errors)} error(s).")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
