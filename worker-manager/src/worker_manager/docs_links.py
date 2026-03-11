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

_DIRECT_DOC_PATHS = (
    "README.md",
    "CONTRIBUTING.md",
    "protocol/README.md",
    "worker-manager/README.md",
    "worker-manager/ADAPTERS.md",
)


@dataclass(frozen=True)
class LinkError:
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
    for raw_target in _LINK_RE.findall(markdown_text):
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


def discover_markdown_files(repo_root: Path) -> list[Path]:
    files: set[Path] = set()
    for relative in _DIRECT_DOC_PATHS:
        path = repo_root / relative
        if path.exists():
            files.add(path)
    docs_dir = repo_root / "docs"
    if docs_dir.exists():
        files.update(path for path in docs_dir.rglob("*.md") if path.is_file())
    return sorted(files)


def validate_markdown_files(files: list[Path], repo_root: Path) -> list[LinkError]:
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
                resolved = (source_file.parent / path_part).resolve()
            else:
                resolved = source_file.resolve()

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
                        f"anchor not found in {resolved.relative_to(repo_root)}",
                    )
                )
    return errors


def validate_repo_docs(repo_root: Path) -> list[LinkError]:
    files = discover_markdown_files(repo_root)
    return validate_markdown_files(files, repo_root)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[3],
        help="Tanren repository root path",
    )
    args = parser.parse_args()
    repo_root = args.repo_root.resolve()

    errors = validate_repo_docs(repo_root)
    if not errors:
        print(f"Docs link check passed ({len(discover_markdown_files(repo_root))} files).")
        return 0

    for error in errors:
        rel_source = error.source_file.relative_to(repo_root)
        print(f"{rel_source}: {error.message} [{error.target}]")
    print(f"Docs link check failed with {len(errors)} error(s).")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
