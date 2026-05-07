---
schema: tanren.experience_flows.v0
status: draft
owner_command: design-experience
updated_at: 2026-05-07
---

# Experience Flows

Per-(behavior × surface) experience contract: entry → primary flow →
success → key failure states → recovery. Behaviors inherit the
surface-default values below unless an explicit row overrides them.

## Source Of Truth

The authoritative behavior catalog is `docs/behaviors/B-*.md`. The
authoritative surface registry is `docs/experience/surfaces.yml`. The
(behavior × surface) inventory is mechanically derivable from those two
sources and should not be duplicated here. This file owns:

- the **row schema** for an experience flow contract;
- **surface defaults** that every behavior inherits;
- **explicit deviations** from defaults that need to be reviewed before
  the corresponding roadmap node is shaped.

When a roadmap node is shaped (`craft-roadmap` → `shape-spec`), the
spec evidence captures the per-behavior flow contract for the
behaviors it completes. Until typed Tanren storage lands, this
projection stays compact on purpose.

## Row Schema

Every flow row records:

| Field | Meaning |
|---|---|
| `behavior` | `B-XXXX` — must exist in `docs/behaviors/`. |
| `surface` | One of the `id:` values in `docs/experience/surfaces.yml`. |
| `entry` | The discoverable affordance (route, command, tool, screen, endpoint). |
| `primary flow` | One sentence summarizing the observable progression. |
| `success` | The state a user or client observes when the behavior completes. |
| `key failures` | Surface-native failure modes a reviewer should expect. |
| `recovery` | What the user or client does when a failure mode appears. |
| `design pattern refs` | Pattern IDs from `docs/experience/design-system/patterns.yml`. |

Cells use `;` as a list separator (Markdown tables treat `|` as a
cell boundary). Pattern references are optional only when the contract records
an explicit design-system deviation.

## Surface Defaults

Every (behavior, surface) pair inherits these defaults unless an
explicit override row appears under "Deviations" below.

| Surface | Entry | Success | Key failures | Recovery |
|---|---|---|---|---|
| `web` | route under `/<area>/…` | visible confirmed state + next-step affordance | `validation_failed` (400); `permission_denied`; `unavailable` | inline correction + retry; no full-page reset |
| `api` | `/<area>/…` endpoint family | stable response body + idempotent state mutation | `validation_failed` (400); `permission_denied` (403); `unavailable` (503) | idempotent retry on 5xx; cursor resume on stale |
| `mcp` | `tanren.<area>.*` tool family | tool result with structured payload + audit attribution | structured error: `validation_failed`; `permission_denied`; `unavailable` | retry with corrected args; surface escalation note |
| `cli` | `tanren <area> …` command family | stdout success line + exit 0 (or JSON success envelope under `--json`) | stderr message + non-zero exit (2 validation, 3 permission, 4 unavailable) | `--help` references corrective flags; `--json` for machines |
| `tui` | `<area>` screen / panel | screen reflects new state + status-bar confirmation | inline error + focus retained + non-destructive recovery | `?` opens contextual help; `Esc` cancels safely |

## Representative Rows

Sampled across surfaces and areas to illustrate the row format and
show how surface-default text reads against real behavior intent.
These are not exhaustive; they exist so a reviewer can understand the
contract without scanning the full behavior catalog.

| Behavior | Title | Surface | Entry | Primary flow | Success | Key failures | Recovery |
|---|---|---|---|---|---|---|---|
| `B-0001` | Start an implementation loop manually on a spec | `web` | route under `/implementation-loop/…` | A `solo-builder` or `team-builder` can start an implementation loop on a selected spec so that Tanren begins… | visible confirmed state + next-step affordance | `validation_failed` (400); `permission_denied`; `unavailable` | inline correction + retry; no full-page reset |
| `B-0289` | Define and maintain project surfaces | `web` | route under `/experience-design/…` | A `solo-builder` or `team-builder` can declare, revise, and retire the public human and machine-facing surfac… | visible confirmed state + next-step affordance | `validation_failed` (400); `permission_denied`; `unavailable` | inline correction + retry; no full-page reset |
| `B-0001` | Start an implementation loop manually on a spec | `api` | `/implementation-loop/…` endpoint family | A `solo-builder` or `team-builder` can start an implementation loop on a selected spec so that Tanren begins… | stable response body + idempotent state mutation | `validation_failed` (400); `permission_denied` (403); `unavailable` (503) | idempotent retry on 5xx; cursor resume on stale |
| `B-0185` | Receive consistent validation errors across public interfac… | `api` | `/cross-interface/…` endpoint family | A user, integration client, or agent worker can receive consistent validation errors across public interfaces… | stable response body + idempotent state mutation | `validation_failed` (400); `permission_denied` (403); `unavailable` (503) | idempotent retry on 5xx; cursor resume on stale |
| `B-0001` | Start an implementation loop manually on a spec | `mcp` | `tanren.implementation-loop.*` tool family | A `solo-builder` or `team-builder` can start an implementation loop on a selected spec so that Tanren begins… | tool result with structured payload + audit attribution | structured error: `validation_failed`; `permission_denied`; `unavailable` | retry with corrected args; surface escalation note |
| `B-0292` | Reject behavior or roadmap drift from the project surface r… | `mcp` | `tanren.experience-design.*` tool family | A `solo-builder` or `team-builder` can rely on Tanren to reject behavior catalog or roadmap edits that refere… | tool result with structured payload + audit attribution | structured error: `validation_failed`; `permission_denied`; `unavailable` | retry with corrected args; surface escalation note |
| `B-0001` | Start an implementation loop manually on a spec | `cli` | `tanren implementation-loop …` command family | A `solo-builder` or `team-builder` can start an implementation loop on a selected spec so that Tanren begins… | stdout success line + exit 0 (or JSON success envelope under `--json`) | stderr message + non-zero exit (2 validation, 3 permission, 4 unavailable) | `--help` references corrective flags; `--json` for machines |
| `B-0184` | Continue the same work from another interface | `cli` | `tanren cross-interface …` command family | A user can continue the same project, spec, loop, or review from another interface so work is not trapped in… | stdout success line + exit 0 (or JSON success envelope under `--json`) | stderr message + non-zero exit (2 validation, 3 permission, 4 unavailable) | `--help` references corrective flags; `--json` for machines |
| `B-0001` | Start an implementation loop manually on a spec | `tui` | `implementation-loop` screen / panel | A `solo-builder` or `team-builder` can start an implementation loop on a selected spec so that Tanren begins… | screen reflects new state + status-bar confirmation | inline error + focus retained + non-destructive recovery | `?` opens contextual help; `Esc` cancels safely |
| `B-0290` | Design experience contracts for behavior-surface pairs | `tui` | `experience-design` screen / panel | A `solo-builder` or `team-builder` can produce experience contracts — entry points, primary flows, observable… | screen reflects new state + status-bar confirmation | inline error + focus retained + non-destructive recovery | `?` opens contextual help; `Esc` cancels safely |

## Deviations

When a behavior needs surface-specific flow text that diverges from
the default row above, record it here as one row. A row in this
section signals to `craft-roadmap` that the corresponding node has
higher experience risk than its declared `experience_risk` would
otherwise suggest.

| Behavior | Surface | Entry | Primary flow | Success | Key failures | Recovery | Reason |
|---|---|---|---|---|---|---|---|
| _none yet_ | | | | | | | |

## Coverage Audit

`docs/behaviors/` contains 288 accepted behaviors.
Each behavior's `surfaces:` frontmatter implicitly creates a flow
contract row by inheriting the surface default and relevant design-system
pattern defaults. Explicit deviations live above; the implicit set is
enumerated by
`scripts/roadmap_check.py` (or the eventual coverage validator
tracked by `B-0291`).
