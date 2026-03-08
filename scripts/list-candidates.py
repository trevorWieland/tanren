#!/usr/bin/env python3
"""List unblocked spec candidates from GitHub issues.

Resolves dependency chains by checking which specs are closed (done),
then filters planned specs to those with all dependencies satisfied.
By default, only shows the earliest milestone. Pass --all to see everything.

Usage:
    python3 tanren/scripts/list-candidates.py          # earliest milestone only
    python3 tanren/scripts/list-candidates.py --all    # all milestones
"""

import json
import operator
import re
import subprocess
import sys


def gh_issues(labels: list[str], state: str = "open", limit: int = 200) -> list[dict]:
    """Fetch issues from GitHub with given labels.

    Returns:
        List of issue dicts with number, title, milestone, and body.
    """
    cmd = [
        "gh",
        "issue",
        "list",
        "--state",
        state,
        "--json",
        "number,title,milestone,body",
        "--limit",
        str(limit),
    ]
    for label in labels:
        cmd.extend(["--label", label])
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Error fetching issues: {result.stderr}", file=sys.stderr)
        sys.exit(1)
    return json.loads(result.stdout)


def extract_spec_id(body: str) -> str | None:
    """Extract spec_id from issue body frontmatter.

    Returns:
        The spec_id string (e.g. "s0.1.03") or None if not found.
    """
    m = re.search(r"spec_id:\s*(s[\d.]+)", body)
    return m.group(1) if m else None


def extract_deps(body: str) -> list[str]:
    """Extract depends_on list from issue body frontmatter.

    Returns:
        List of spec_id strings this spec depends on.
    """
    m = re.search(r"depends_on:\s*\[([^\]]*)\]", body)
    if not m:
        return []
    return [d.strip() for d in m.group(1).split(",") if d.strip()]


def main() -> None:
    """Find and display unblocked spec candidates."""
    show_all = "--all" in sys.argv

    planned = gh_issues(["type:spec", "status:planned"], state="open")
    closed = gh_issues(["type:spec"], state="closed")

    done_ids = {extract_spec_id(c.get("body", "")) for c in closed} - {None}

    candidates = []
    for p in planned:
        body = p.get("body", "")
        spec_id = extract_spec_id(body) or "?"
        deps = extract_deps(body)
        milestone = (p.get("milestone") or {}).get("title", "none")

        blocked_by = [d for d in deps if d not in done_ids]

        if not blocked_by:
            candidates.append((milestone, spec_id, p["number"], p["title"]))

    candidates.sort(key=operator.itemgetter(0, 1))

    if not candidates:
        print("No unblocked specs found.")
        sys.exit(0)

    # Filter to earliest milestone unless --all
    if not show_all:
        earliest = candidates[0][0]
        candidates = [c for c in candidates if c[0] == earliest]

    milestone_label = candidates[0][0]
    print(f"Unblocked candidates — {milestone_label} ({len(candidates)}):\n")
    for _milestone, _spec_id, number, title in candidates:
        print(f"  #{number} {title}")

    if not show_all:
        print("\n(Pass --all to see candidates from later milestones too)")


if __name__ == "__main__":
    main()
