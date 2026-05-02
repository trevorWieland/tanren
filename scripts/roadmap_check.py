#!/usr/bin/env python3
"""roadmap_check.py — validate and analyze docs/roadmap/dag.json.

Checks the roadmap DAG for structural and semantic problems and reports
parallelism / depth metrics. Exits non-zero if any check fails.

Usage:
    # Default: full validation + report
    python3 scripts/roadmap_check.py
    python3 scripts/roadmap_check.py --verbose       # level-by-level layout
    python3 scripts/roadmap_check.py --reduce        # remove redundant edges
    python3 scripts/roadmap_check.py --path PATH     # alternate dag.json

    # Focused views (skip the default report)
    python3 scripts/roadmap_check.py --ready             # nodes ready to start now
    python3 scripts/roadmap_check.py --milestone M-XXXX  # one milestone's slice
    python3 scripts/roadmap_check.py --node R-XXXX       # one node's full info + neighbors
    python3 scripts/roadmap_check.py --behavior B-XXXX   # which node completes/supports a behavior
    python3 scripts/roadmap_check.py --critical-path     # longest dependency chain
    python3 scripts/roadmap_check.py --coverage-map      # behavior → node mapping

Checks:
  - Schema (required top-level keys, required node fields)
  - Unique node and milestone IDs
  - Each node references an existing milestone
  - Each `depends_on` references an existing node
  - Each `completes_behaviors` / `supports_behaviors` references an
    accepted behavior under docs/behaviors
  - Each behavior node completes at least one behavior (foundation kind exempt)
  - No behavior is completed by more than one node
  - DAG is acyclic
  - Every behavior node has the foundation spec as a transitive ancestor
  - Each `expected_evidence[].interfaces` matches the behavior's frontmatter
    `interfaces:` declaration (catches drift between catalog and DAG)
  - Each `tests/bdd/features/B-XXXX-*.feature` file references a behavior
    that has a corresponding R-* node `expected_evidence` entry
    (inverse of the `xtask check-bdd-tags` cross-check; catches deletes
    or renames that orphan a feature file from the DAG)
  - (Warn) No transitively redundant `depends_on` edges
  - (Warn) Playbook count vs. declared interfaces — flags suspiciously thin
    playbooks for nodes with 5 declared interfaces
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict, deque
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DAG_PATH = REPO_ROOT / "docs" / "roadmap" / "dag.json"
BEHAVIORS_DIR = REPO_ROOT / "docs" / "behaviors"
FEATURES_DIR = REPO_ROOT / "tests" / "bdd" / "features"

NODE_REQUIRED = (
    "id",
    "title",
    "milestone",
    "completes_behaviors",
    "depends_on",
    "scope",
)
MILESTONE_REQUIRED = ("id", "title", "goal", "status")
NODE_KINDS = {"behavior", "foundation"}


def load_dag(path: Path) -> dict[str, Any]:
    with path.open() as f:
        return json.load(f)


def collect_behaviors() -> tuple[set[str], set[str], dict[str, set[str]]]:
    """Parse docs/behaviors/B-*.md.

    Returns (accepted_ids, deprecated_ids, interfaces_by_id) where
    interfaces_by_id is the frontmatter `interfaces:` list per behavior, used
    by the evidence/interface-alignment check.
    """
    accepted: set[str] = set()
    deprecated: set[str] = set()
    interfaces: dict[str, set[str]] = {}
    id_re = re.compile(r"^id:\s*(B-\d{4})", re.MULTILINE)
    status_re = re.compile(r"^product_status:\s*(\w+)", re.MULTILINE)
    iface_re = re.compile(r"^interfaces:\s*\[([^\]]*)\]", re.MULTILINE)
    for f in sorted(BEHAVIORS_DIR.glob("B-*.md")):
        text = f.read_text()
        id_m = id_re.search(text)
        st_m = status_re.search(text)
        if not id_m or not st_m:
            continue
        bid = id_m.group(1)
        if st_m.group(1) == "accepted":
            accepted.add(bid)
        elif st_m.group(1) == "deprecated":
            deprecated.add(bid)
        if_m = iface_re.search(text)
        if if_m:
            items = {p.strip() for p in if_m.group(1).split(",") if p.strip()}
            interfaces[bid] = items
    return accepted, deprecated, interfaces


def check_schema(dag: dict, errors: list[str]) -> tuple[set[str], set[str]]:
    """Validate top-level + per-record fields. Return (milestone_ids, node_ids)."""
    for k in ("schema", "milestones", "nodes"):
        if k not in dag:
            errors.append(f"top-level: missing key {k!r}")

    milestone_ids: set[str] = set()
    for m in dag.get("milestones", []):
        for k in MILESTONE_REQUIRED:
            if k not in m:
                errors.append(f"milestone {m.get('id', '<?>')}: missing {k!r}")
        mid = m.get("id")
        if mid:
            if mid in milestone_ids:
                errors.append(f"duplicate milestone id: {mid}")
            milestone_ids.add(mid)

    node_ids: set[str] = set()
    for n in dag.get("nodes", []):
        for k in NODE_REQUIRED:
            if k not in n:
                errors.append(f"node {n.get('id', '<?>')}: missing {k!r}")
        nid = n.get("id")
        if nid:
            if nid in node_ids:
                errors.append(f"duplicate node id: {nid}")
            node_ids.add(nid)
        kind = n.get("kind", "behavior")
        if kind not in NODE_KINDS:
            errors.append(f"node {nid}: unknown kind {kind!r}")
        # Behavior nodes must complete at least one behavior. Foundation
        # nodes are scaffolding and may complete zero.
        if kind == "behavior" and not n.get("completes_behaviors"):
            errors.append(f"node {nid}: completes_behaviors must be non-empty")
        if n.get("milestone") and n["milestone"] not in milestone_ids:
            errors.append(
                f"node {nid}: references unknown milestone {n['milestone']!r}"
            )
    return milestone_ids, node_ids


def check_evidence_interfaces(
    dag: dict,
    behavior_interfaces: dict[str, set[str]],
    errors: list[str],
) -> None:
    """Each evidence item's `interfaces` must equal the behavior's frontmatter."""
    for n in dag.get("nodes", []):
        nid = n.get("id", "<?>")
        for ev in n.get("expected_evidence", []) or []:
            bid = ev.get("behavior_id")
            if not bid or bid not in behavior_interfaces:
                continue
            declared = behavior_interfaces[bid]
            claimed = set(ev.get("interfaces") or [])
            if declared != claimed:
                missing = declared - claimed
                extra = claimed - declared
                parts = []
                if missing:
                    parts.append(f"missing {sorted(missing)}")
                if extra:
                    parts.append(f"unexpected {sorted(extra)}")
                errors.append(
                    f"{nid}: evidence for {bid} interfaces {sorted(claimed)} "
                    f"do not match behavior frontmatter {sorted(declared)} "
                    f"({'; '.join(parts)})"
                )


def check_feature_files(
    dag: dict,
    accepted: set[str],
    errors: list[str],
) -> None:
    """Each `B-XXXX-*.feature` file under tests/bdd/features must point at
    an accepted behavior that some R-* node lists in its
    `expected_evidence`. F-0002 BDD convention says one feature per
    behavior; this check is the inverse of `xtask check-bdd-tags` and
    catches the case where a feature exists but no DAG node owns it.
    """
    if not FEATURES_DIR.exists():
        return
    name_re = re.compile(r"^B-(\d{4})-")
    evidence_owners: dict[str, str] = {}
    for n in dag.get("nodes", []):
        nid = n.get("id", "<?>")
        for ev in n.get("expected_evidence", []) or []:
            bid = ev.get("behavior_id")
            if bid:
                evidence_owners[bid] = nid
    for f in sorted(FEATURES_DIR.glob("B-*.feature")):
        m = name_re.match(f.name)
        if not m:
            errors.append(
                f"{f.relative_to(REPO_ROOT)}: filename does not match "
                "B-XXXX-<slug>.feature"
            )
            continue
        bid = f"B-{m.group(1)}"
        if bid not in accepted:
            errors.append(
                f"{f.relative_to(REPO_ROOT)}: behavior {bid} is not "
                "accepted in docs/behaviors"
            )
            continue
        if bid not in evidence_owners:
            errors.append(
                f"{f.relative_to(REPO_ROOT)}: behavior {bid} has a feature "
                "file but no DAG node lists it in expected_evidence"
            )


def find_thin_playbooks(
    dag: dict,
    behavior_interfaces: dict[str, set[str]],
) -> list[tuple[str, int, int]]:
    """Flag nodes whose playbook is suspiciously thin for their interface span.

    Returns (node_id, playbook_count, distinct_interface_count) for each node
    where the playbook has fewer entries than the count of distinct interfaces
    spanned by its `expected_evidence` AND the average playbook entry length
    is under 30 characters. Foundation nodes are exempt.
    """
    thin: list[tuple[str, int, int]] = []
    for n in dag.get("nodes", []):
        if n.get("kind") == "foundation":
            continue
        pb = n.get("playbook") or []
        if not pb:
            continue
        ifaces: set[str] = set()
        for ev in n.get("expected_evidence", []) or []:
            for i in ev.get("interfaces") or []:
                ifaces.add(i)
        if not ifaces:
            continue
        avg_len = sum(len(p) for p in pb) / len(pb)
        if len(pb) < len(ifaces) and avg_len < 30:
            thin.append((n["id"], len(pb), len(ifaces)))
    return thin


def check_references(
    dag: dict,
    accepted: set[str],
    deprecated: set[str],
    node_ids: set[str],
    errors: list[str],
) -> None:
    """Validate behavior + dep references against the universe."""
    completes_owner: dict[str, str] = {}
    for n in dag.get("nodes", []):
        nid = n.get("id", "<?>")
        for b in n.get("completes_behaviors", []):
            if b in deprecated:
                errors.append(f"{nid}: completes deprecated behavior {b}")
            elif b not in accepted:
                errors.append(f"{nid}: completes unknown behavior {b}")
            if b in completes_owner:
                errors.append(
                    f"behavior {b} is completed by both "
                    f"{completes_owner[b]} and {nid}"
                )
            else:
                completes_owner[b] = nid
        for b in n.get("supports_behaviors", []):
            if b in deprecated:
                errors.append(f"{nid}: supports deprecated behavior {b}")
            elif b not in accepted:
                errors.append(f"{nid}: supports unknown behavior {b}")
        for d in n.get("depends_on", []):
            if d not in node_ids:
                errors.append(f"{nid}: depends_on unknown node {d}")


def topo_sort(
    nodes: dict[str, dict], errors: list[str]
) -> list[str] | None:
    in_degree = {nid: 0 for nid in nodes}
    children: dict[str, list[str]] = defaultdict(list)
    for nid, n in nodes.items():
        for d in n.get("depends_on", []):
            if d not in nodes:
                continue
            children[d].append(nid)
            in_degree[nid] += 1
    queue = deque(nid for nid, d in in_degree.items() if d == 0)
    order: list[str] = []
    while queue:
        n = queue.popleft()
        order.append(n)
        for c in children[n]:
            in_degree[c] -= 1
            if in_degree[c] == 0:
                queue.append(c)
    if len(order) != len(nodes):
        unprocessed = sorted(nid for nid, d in in_degree.items() if d > 0)
        errors.append(f"cycle detected involving {unprocessed}")
        return None
    return order


def compute_depths(
    nodes: dict[str, dict], topo: list[str]
) -> dict[str, int]:
    depth = {nid: 0 for nid in nodes}
    for nid in topo:
        for d in nodes[nid].get("depends_on", []):
            if d in depth:
                depth[nid] = max(depth[nid], depth[d] + 1)
    return depth


def compute_ancestors(
    nodes: dict[str, dict], topo: list[str]
) -> dict[str, set[str]]:
    """ancestors[n] = transitive set of nodes n depends on (excluding n)."""
    anc: dict[str, set[str]] = {nid: set() for nid in nodes}
    for nid in topo:
        for d in nodes[nid].get("depends_on", []):
            if d in anc:
                anc[nid].add(d)
                anc[nid].update(anc[d])
    return anc


def find_redundant_edges(
    nodes: dict[str, dict], ancestors: dict[str, set[str]]
) -> list[tuple[str, str, str]]:
    """Return (u, v, w) where u→v is implied by u→w→…→v."""
    redundant: list[tuple[str, str, str]] = []
    for nid, n in nodes.items():
        deps = list(n.get("depends_on", []))
        for v in deps:
            for w in deps:
                if w == v:
                    continue
                if v in ancestors.get(w, set()):
                    redundant.append((nid, v, w))
                    break
    return redundant


def reduce_dag_in_place(dag: dict, redundant: list[tuple[str, str, str]]) -> int:
    """Remove redundant depends_on edges. Return count removed."""
    removals: dict[str, set[str]] = defaultdict(set)
    for u, v, _w in redundant:
        removals[u].add(v)
    removed = 0
    for n in dag.get("nodes", []):
        rs = removals.get(n["id"])
        if not rs:
            continue
        before = list(n.get("depends_on", []))
        after = [d for d in before if d not in rs]
        n["depends_on"] = after
        removed += len(before) - len(after)
    return removed


def view_ready(
    nodes_by_id: dict[str, dict], depths: dict[str, int]
) -> None:
    """List nodes ready to start now: status='complete' deps only."""
    ready: list[str] = []
    for nid, n in nodes_by_id.items():
        if n.get("status", "planned") == "complete":
            continue
        deps_ok = all(
            nodes_by_id.get(d, {}).get("status", "planned") == "complete"
            for d in n.get("depends_on", [])
        )
        if deps_ok:
            ready.append(nid)
    ready.sort(key=lambda r: (depths.get(r, 0), r))
    if not ready:
        print("No nodes are ready (all are blocked, complete, or absent).")
        return
    print(f"Ready to start ({len(ready)}):")
    for nid in ready:
        n = nodes_by_id[nid]
        kind = n.get("kind", "behavior")
        completes = ",".join(n.get("completes_behaviors", [])) or "—"
        print(
            f"  {nid}  L{depths.get(nid, '?'):>2}  {kind:>10}  "
            f"[{completes}]  {n.get('title', '')}"
        )


def view_milestone(dag: dict, milestone_id: str, depths: dict[str, int]) -> int:
    nodes = [n for n in dag.get("nodes", []) if n.get("milestone") == milestone_id]
    if not nodes:
        print(f"No nodes in milestone {milestone_id}")
        return 1
    m = next((m for m in dag.get("milestones", []) if m.get("id") == milestone_id), None)
    title = m.get("title", "") if m else ""
    goal = m.get("goal", "") if m else ""
    print(f"{milestone_id}  {title}")
    print(f"Goal: {goal}\n")
    nodes.sort(key=lambda n: (depths.get(n["id"], 0), n["id"]))
    for n in nodes:
        nid = n["id"]
        kind = n.get("kind", "behavior")
        completes = ",".join(n.get("completes_behaviors", [])) or "—"
        deps = ",".join(n.get("depends_on", [])) or "—"
        print(
            f"  {nid}  L{depths.get(nid, '?'):>2}  {kind:>10}  "
            f"completes=[{completes}]"
        )
        print(f"        {n.get('title', '')}")
        print(f"        depends_on=[{deps}]")
    print(f"\nTotal: {len(nodes)} nodes")
    return 0


def view_node(
    dag: dict, node_id: str, ancestors: dict[str, set[str]]
) -> int:
    nodes_by_id = {n["id"]: n for n in dag.get("nodes", []) if "id" in n}
    n = nodes_by_id.get(node_id)
    if not n:
        print(f"Node {node_id} not found")
        return 1
    children: list[str] = sorted(
        nid for nid, m in nodes_by_id.items()
        if node_id in m.get("depends_on", [])
    )
    direct_deps = list(n.get("depends_on", []))
    transitive_deps = ancestors.get(node_id, set()) - set(direct_deps)
    print(f"{node_id}  {n.get('title', '')}")
    print(f"  milestone:    {n.get('milestone', '')}")
    print(f"  kind:         {n.get('kind', 'behavior')}")
    print(f"  status:       {n.get('status', 'planned')}")
    print(f"  completes:    {', '.join(n.get('completes_behaviors', []))}")
    if n.get("supports_behaviors"):
        print(f"  supports:     {', '.join(n.get('supports_behaviors', []))}")
    print(f"\n  Direct deps   ({len(direct_deps)}):  {', '.join(direct_deps) or '—'}")
    print(f"  Transitive    ({len(transitive_deps)}):  {', '.join(sorted(transitive_deps)) or '—'}")
    print(f"  Direct children ({len(children)}): {', '.join(children) or '—'}")
    print(f"\n  Scope:")
    for line in (n.get("scope") or "").splitlines() or ["(none)"]:
        print(f"    {line}")
    return 0


def view_behavior(dag: dict, behavior_id: str) -> int:
    completers: list[str] = []
    supporters: list[str] = []
    for n in dag.get("nodes", []):
        if behavior_id in n.get("completes_behaviors", []):
            completers.append(n["id"])
        if behavior_id in n.get("supports_behaviors", []):
            supporters.append(n["id"])
    if not completers and not supporters:
        # Check if behavior exists in catalog
        accepted, deprecated, _ = collect_behaviors()
        if behavior_id in deprecated:
            print(f"{behavior_id}: deprecated — no node should reference it")
            return 0
        if behavior_id in accepted:
            print(f"{behavior_id}: NOT YET COVERED in roadmap")
            return 0
        print(f"{behavior_id}: not found in catalog")
        return 1
    print(f"{behavior_id}")
    print(f"  Completed by: {', '.join(completers) or '(uncovered)'}")
    print(f"  Supported by: {', '.join(supporters) or '—'}")
    return 0


def view_coverage_map(dag: dict, accepted: set[str]) -> None:
    completers: dict[str, str] = {}
    for n in dag.get("nodes", []):
        for b in n.get("completes_behaviors", []):
            completers[b] = n["id"]
    print("Behavior → node mapping:")
    for b in sorted(accepted):
        owner = completers.get(b, "(uncovered)")
        print(f"  {b}  →  {owner}")


def view_critical_path(
    nodes_by_id: dict[str, dict], depths: dict[str, int]
) -> None:
    """Print one longest path through the DAG."""
    if not depths:
        print("(empty graph)")
        return
    end = max(depths, key=lambda nid: depths[nid])
    path = [end]
    cur = end
    while True:
        deps = nodes_by_id[cur].get("depends_on", [])
        if not deps:
            break
        # Pick the dep with the greatest depth (it's on the longest path).
        nxt = max(deps, key=lambda d: depths.get(d, -1))
        path.append(nxt)
        cur = nxt
    path.reverse()
    print(f"Critical path ({len(path)} nodes, depth {depths[end] + 1}):")
    for i, nid in enumerate(path):
        n = nodes_by_id[nid]
        print(
            f"  {i+1:>2}. {nid}  ({n.get('milestone', '?')})  "
            f"{n.get('title', '')}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--path", default=str(DEFAULT_DAG_PATH))
    parser.add_argument(
        "--reduce", action="store_true",
        help="Rewrite dag.json with redundant depends_on removed",
    )
    parser.add_argument(
        "--verbose", action="store_true",
        help="Print level-by-level node layout",
    )
    parser.add_argument(
        "--ready", action="store_true",
        help="List nodes whose deps are all complete (ready to start)",
    )
    parser.add_argument(
        "--milestone", metavar="M-XXXX",
        help="Show one milestone's slice with internal layout",
    )
    parser.add_argument(
        "--node", metavar="R-XXXX",
        help="Show one node's full info plus upstream/downstream",
    )
    parser.add_argument(
        "--behavior", metavar="B-XXXX",
        help="Show which node completes or supports a behavior",
    )
    parser.add_argument(
        "--coverage-map", action="store_true",
        help="Print every accepted behavior with its completing node",
    )
    parser.add_argument(
        "--critical-path", action="store_true",
        help="Print one longest dependency path through the DAG",
    )
    args = parser.parse_args()

    path = Path(args.path)
    if not path.exists():
        print(f"FAIL: {path} does not exist", file=sys.stderr)
        return 2

    dag = load_dag(path)
    accepted, deprecated, behavior_interfaces = collect_behaviors()

    errors: list[str] = []
    milestone_ids, node_ids = check_schema(dag, errors)
    check_references(dag, accepted, deprecated, node_ids, errors)
    check_evidence_interfaces(dag, behavior_interfaces, errors)
    check_feature_files(dag, accepted, errors)

    nodes_by_id = {n["id"]: n for n in dag.get("nodes", []) if "id" in n}
    topo = topo_sort(nodes_by_id, errors) if not errors else None
    depths = compute_depths(nodes_by_id, topo) if topo else {}
    ancestors = compute_ancestors(nodes_by_id, topo) if topo else {}
    redundant = find_redundant_edges(nodes_by_id, ancestors) if topo else []
    thin_playbooks = find_thin_playbooks(dag, behavior_interfaces)

    # Foundation reachability: every behavior node should transitively
    # depend on the foundation spec (if one is declared).
    foundation_id = dag.get("foundation_spec_id")
    if foundation_id and topo:
        if foundation_id not in nodes_by_id:
            errors.append(
                f"foundation_spec_id {foundation_id!r} is not a declared node"
            )
        else:
            for nid, n in nodes_by_id.items():
                if nid == foundation_id:
                    continue
                if n.get("kind", "behavior") != "behavior":
                    continue
                if foundation_id not in ancestors.get(nid, set()):
                    errors.append(
                        f"{nid}: behavior node has no path back to "
                        f"foundation {foundation_id}"
                    )

    completes = {
        b
        for n in dag.get("nodes", [])
        for b in n.get("completes_behaviors", [])
    }
    covered = completes & accepted
    uncovered = accepted - completes

    # ---- focused views (skip default report) ----
    focused = (
        args.ready or args.milestone or args.node or args.behavior
        or args.coverage_map or args.critical_path
    )
    if focused:
        if errors:
            print(f"WARN: structural errors present ({len(errors)}); view may be partial")
            for e in errors[:5]:
                print(f"  {e}")
            print()
        if args.ready:
            view_ready(nodes_by_id, depths)
        if args.milestone:
            view_milestone(dag, args.milestone, depths)
        if args.node:
            view_node(dag, args.node, ancestors)
        if args.behavior:
            view_behavior(dag, args.behavior)
        if args.coverage_map:
            view_coverage_map(dag, accepted)
        if args.critical_path:
            view_critical_path(nodes_by_id, depths)
        return 1 if errors else 0

    # ---- report ----
    n_milestones = len(dag.get("milestones", []))
    n_nodes = len(dag.get("nodes", []))
    print(f"Roadmap: {n_milestones} milestones, {n_nodes} spec nodes")
    print(
        f"Behaviors: {len(accepted)} accepted; "
        f"{len(covered)} covered; {len(uncovered)} uncovered"
    )

    if uncovered and args.verbose:
        print(f"\nUncovered ({len(uncovered)}):")
        for b in sorted(uncovered):
            print(f"  {b}")

    if topo and depths:
        max_depth = max(depths.values()) + 1 if depths else 0
        levels: dict[int, list[str]] = defaultdict(list)
        for nid, d in depths.items():
            levels[d].append(nid)
        max_width = max((len(v) for v in levels.values()), default=0)
        print(f"\nLongest path: {max_depth} nodes")
        print(f"Max parallel width: {max_width} nodes")
        if args.verbose:
            print("\nLevel layout:")
            for d in sorted(levels):
                ids = sorted(levels[d])
                print(f"  L{d:>2}  ({len(ids):>3})  {', '.join(ids)}")

    if redundant:
        print(f"\nRedundant edges ({len(redundant)}):")
        for u, v, w in redundant[:20]:
            print(f"  {u} → {v}  (already implied via {w})")
        if len(redundant) > 20:
            print(f"  … {len(redundant) - 20} more")

    if thin_playbooks:
        print(
            f"\nThin playbooks ({len(thin_playbooks)}): "
            "playbook count < distinct interface count and avg entry < 30 chars"
        )
        if args.verbose:
            for nid, pb_count, iface_count in thin_playbooks[:20]:
                print(
                    f"  {nid}  playbook={pb_count} interfaces={iface_count}"
                )
            if len(thin_playbooks) > 20:
                print(f"  … {len(thin_playbooks) - 20} more")
        else:
            print("  (run with --verbose to list)")

    print("\nPer-milestone:")
    counts_n = defaultdict(int)
    counts_b = defaultdict(int)
    for n in dag.get("nodes", []):
        counts_n[n.get("milestone")] += 1
        counts_b[n.get("milestone")] += len(n.get("completes_behaviors", []))
    for m in dag.get("milestones", []):
        mid = m.get("id", "<?>")
        print(
            f"  {mid}  {counts_n.get(mid, 0):>3} nodes  "
            f"{counts_b.get(mid, 0):>3} behaviors  {m.get('title', '')}"
        )

    if args.reduce and redundant:
        removed = reduce_dag_in_place(dag, redundant)
        with path.open("w") as f:
            json.dump(dag, f, indent=2)
            f.write("\n")
        print(f"\nReduced: removed {removed} redundant edges from {path}")
        return 0

    if errors:
        print(f"\nErrors ({len(errors)}):")
        for e in errors:
            print(f"  {e}")
        return 1

    if redundant:
        print("\nWARN: redundant edges present — run with --reduce to remove")
        return 0

    print("\nOK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
