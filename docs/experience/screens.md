---
schema: tanren.experience_screens.v0
status: draft
owner_command: design-experience
updated_at: 2026-05-07
---

# Experience Screens And Surface Inventory

Surface-keyed inventory of routes, endpoints, tools, commands, and
screens, grouped by behavior `area`. Each row is a *family* — real
route, command, and tool naming lands when the corresponding
roadmap node is shaped. The per-behavior membership of each family
is derivable from `docs/behaviors/B-*.md` `surfaces:` frontmatter and
is not duplicated here.

Counts are recomputed when behaviors land; a coverage validator
(tracked by `B-0291`) will eventually keep them honest.

| Area | web routes | api endpoints | mcp tools | cli commands | tui screens |
|---|---:|---:|---:|---:|---:|
| `architecture-planning` | 2 | 2 | 2 | 2 | 2 |
| `autonomy-controls` | 4 | 4 | 4 | 4 | 4 |
| `behavior-proof` | 1 | 1 | 1 | 1 | 1 |
| `configuration` | 19 | 19 | 19 | 19 | 19 |
| `cross-interface` | 3 | 3 | 3 | 3 | 3 |
| `decision-memory` | 5 | 5 | 5 | 5 | 5 |
| `experience-design` | 6 | 6 | 6 | 6 | 6 |
| `external-tracker` | 8 | 8 | 8 | 8 | 8 |
| `findings` | 1 | 1 | 1 | 1 | 1 |
| `governance` | 33 | 33 | 33 | 33 | 33 |
| `implementation-assessment` | 2 | 2 | 2 | 2 | 2 |
| `implementation-loop` | 11 | 11 | 11 | 11 | 11 |
| `intake` | 7 | 7 | 7 | 7 | 7 |
| `integration-contract` | 1 | 10 | 10 | 1 | 1 |
| `integration-management` | 9 | 9 | 9 | 9 | 9 |
| `observation` | 23 | 23 | 23 | 23 | 23 |
| `operations` | 11 | 11 | 11 | 11 | 11 |
| `planner-orchestration` | 9 | 9 | 9 | 9 | 9 |
| `prioritization` | 4 | 4 | 4 | 4 | 4 |
| `proactive-analysis` | 3 | 3 | 3 | 3 | 3 |
| `product-discovery` | 5 | 5 | 5 | 5 | 5 |
| `product-planning` | 8 | 8 | 8 | 8 | 8 |
| `project-setup` | 12 | 12 | 12 | 16 | 12 |
| `release-learning` | 6 | 6 | 6 | 6 | 6 |
| `repo-understanding` | 4 | 4 | 4 | 4 | 4 |
| `review-merge` | 11 | 11 | 11 | 11 | 11 |
| `runtime-actor-contract` | — | 12 | 12 | — | — |
| `runtime-substrate` | 14 | 14 | 14 | 14 | 14 |
| `spec-lifecycle` | 11 | 11 | 11 | 11 | 11 |
| `spec-quality` | 5 | 5 | 5 | 5 | 5 |
| `standards-evolution` | 4 | 4 | 4 | 4 | 4 |
| `team-coordination` | 16 | 16 | 16 | 16 | 16 |
| `undo-recovery` | 3 | 3 | 3 | 3 | 3 |
| `walk-acceptance` | 4 | 4 | 4 | 4 | 4 |

## Surface Family Templates

Naming templates for each surface; an `<area>` token expands to the
behavior `area` value. Real names land in spec evidence when the
behavior is shaped.

| Surface | Family template | Example with `area=implementation-loop` |
|---|---|---|
| `web` (routes) | `/<area>/…` | `/implementation-loop/…` |
| `api` (endpoints) | `/<area>/…` | `/implementation-loop/…` |
| `mcp` (tools) | `tanren.<area>.*` | `tanren.implementation-loop.*` |
| `cli` (commands) | `tanren <area> …` | `tanren implementation-loop …` |
| `tui` (screens) | `<area>` screen | `implementation-loop` screen |

## Cross-Surface Notes

- A behavior's `surfaces:` frontmatter selects which family rows it
  belongs to; absence from a surface is not a defect.
- A surface family without any current behaviors does not appear in
  the table above. Adding the first behavior lights up the family.
- Tanren enforces `surfaces.yml` ↔ behavior frontmatter consistency
  via `xtask check-bdd-tags` and `scripts/roadmap_check.py`.
