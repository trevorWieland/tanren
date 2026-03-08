# Sync Roadmap

Sync `tanren/product/roadmap.md` with GitHub issues labeled `type:spec`. Use GitHub as the source of truth for spec ids. Interactive — the user approves all conflict resolutions and new issue/entry creation before any changes are made.

**Suggested model:** Strong reasoner with good judgment (e.g., Opus via TUI). Conflict resolution and dependency alignment require cross-referencing multiple sources and the user must approve actions.

## Important Guidelines

- Human-in-the-loop — the user approves all sync actions before execution
- Always use AskUserQuestion tool when asking the user anything. In TUI: use AskUserQuestion. In Discord/NanoClaw: use send_message with numbered options and wait for reply.
- Prefer GitHub issues for spec id allocation and dependency relationships
- Ask on conflicts — never overwrite silently
- Never modify spec.md files
- Issue bodies must use YAML frontmatter format (Format A)

## When to Use

- After adding new specs to the roadmap that need GitHub issues
- After creating GitHub issues that need roadmap entries
- After closing/reopening issues to sync status back to the roadmap
- Periodically to catch drift between roadmap and GitHub
- After `shape-spec` creates a new spec (to ensure the issue exists and roadmap is updated)

## Prerequisites

1. `tanren/product/roadmap.md` exists and is parseable
2. `gh` CLI is authenticated and can access the repo
3. Repository has `type:spec` label configured

If roadmap is missing, exit with `sync-roadmap-status: error`.
If `gh` auth fails, exit with `sync-roadmap-status: error`.

## Process

### Step 1: Load Sources

1. Read `tanren/product/roadmap.md` and parse spec items:
   - `spec_id`, title, description, `depends_on`, status (✅ = done, ❌ = cancelled, blank = planned), version

2. Fetch all GitHub issues with label `type:spec`:
   ```bash
   gh issue list --label "type:spec" --state all --limit 200 \
     --json number,title,body,state,labels
   ```

3. Parse each issue's YAML frontmatter to extract structured fields:
   ```
   spec_id, version, status, depends_on
   ```
   Extract `spec_id` from frontmatter: look for `spec_id: <value>` between `---` delimiters.
   If no YAML frontmatter exists, attempt to extract `spec_id` from the issue title (e.g., `s0.1.28 OpenRouter Full Support` → `s0.1.28`).

4. Build lookup tables:
   - `spec_id → issue_number` (from GitHub)
   - `spec_id → roadmap_entry` (from roadmap)

### Step 2: Compare and Classify

For each `spec_id`, determine:
- **Match:** present in both sources
- **Roadmap-only:** in roadmap but no GitHub issue
- **GitHub-only:** in GitHub but not in roadmap
- **Conflict:** fields differ (title, status, version, depends_on)

### Step 3: Resolve Conflicts (Ask)

For each conflict, use AskUserQuestion with a recommended default:

```
Conflict for sX.Y.ZZ — {field}:
- Roadmap: {value}
- GitHub:  {value}

Which should we keep?
1. GitHub (Recommended)
2. Roadmap
```

Apply the user's choice per conflict.

### Step 4: Apply Sync Actions

#### Roadmap-only specs → Create GitHub issues

Before creation, verify `spec_id` isn't already used on GitHub.

Create with YAML frontmatter body (Format A):

```bash
gh issue create \
  --title "{spec_id} {title}" \
  --label "type:spec" --label "status:planned" --label "version:{version}" \
  --body-file /tmp/issue_body.md
```

Issue body template:

```markdown
---
spec_id: {spec_id}
version: {version}
status: planned
depends_on: [{comma-separated spec_ids}]
---

{description from roadmap}

## Acceptance Criteria

- [ ] TBD — flesh out via /shape-spec
```

#### GitHub-only specs → Add roadmap entries

Add a roadmap entry under the correct version section, matching the format of existing entries.

#### Matched specs → Ensure alignment

Update roadmap or GitHub fields per conflict resolution decisions.

### Step 5: Dependency Alignment

Dependencies are tracked in two places: `depends_on` in issue body frontmatter, and GitHub's native `blockedBy`/`blocking` issue relationships.

#### Query existing relationships

Get node IDs for all `type:spec` issues (needed for GraphQL):

```bash
gh api graphql -f query='
{
  repository(owner: "{owner}", name: "{repo}") {
    issues(first: 100, labels: ["type:spec"], orderBy: {field: CREATED_AT, direction: ASC}) {
      nodes {
        number
        id
      }
      pageInfo { endCursor hasNextPage }
    }
  }
}'
```

Paginate if `hasNextPage` is true. Build `issue_number → node_id` mapping.

#### Compare dependencies

For each issue:
1. Parse `depends_on` from YAML frontmatter → resolve `spec_id`s to issue numbers
2. Parse `depends_on` from roadmap → resolve `spec_id`s to issue numbers
3. Query existing `blockedBy` relationships from GitHub (via sub-issues API)
4. Identify:
   - **Missing in GitHub:** dependency in frontmatter/roadmap but no `blockedBy` relationship
   - **Missing in frontmatter:** `blockedBy` relationship exists but not in `depends_on`
   - **Stale:** `blockedBy` relationship exists for a dependency no longer in roadmap or frontmatter

#### Add missing relationships

Use the `addBlockedBy` mutation to add missing `blockedBy` relationships:

```bash
gh api graphql -f query='
mutation($issueId: ID!, $blockingIssueId: ID!) {
  addBlockedBy(input: {
    issueId: $issueId,
    blockingIssueId: $blockingIssueId
  }) {
    issue { number }
    blockingIssue { number }
  }
}' -f issueId="{blocked_node_id}" -f blockingIssueId="{blocking_node_id}"
```

Where `issueId` = the blocked issue (dependent), `blockingIssueId` = the blocking issue (dependency).

Note: `addBlockedBy` supports many-to-many relationships (unlike `addSubIssue` which is single-parent).

#### Remove stale relationships

Use `removeBlockedBy` for relationships that no longer match either source:

```bash
gh api graphql -f query='
mutation($issueId: ID!, $blockingIssueId: ID!) {
  removeBlockedBy(input: {
    issueId: $issueId,
    blockingIssueId: $blockingIssueId
  }) {
    issue { number }
    blockingIssue { number }
  }
}' -f issueId="{blocked_node_id}" -f blockingIssueId="{blocking_node_id}"
```

### Step 6: Label Hygiene

Check for and fix label mismatches:

| Condition | Action |
|-----------|--------|
| CLOSED + `status:planned` (not cancelled) | Change label to `status:done` |
| CLOSED + `status:planned` (cancelled in roadmap) | Change label to `status:wontfix` |
| OPEN + `status:done` | Flag for user review |
| `type:spec` but no parseable `spec_id` | Flag for user review |

Status mapping reference:

| Roadmap | GitHub State | GitHub Label | Meaning |
|---------|-------------|-------------|---------|
| ✅ | CLOSED | status:done | Completed |
| ❌ | CLOSED | status:wontfix | Cancelled |
| (blank) | OPEN | status:planned | Planned |

Apply label changes:

```bash
gh issue edit {number} --remove-label "status:planned" --add-label "status:done"
```

### Step 7: Body Format Normalization

Detect issues missing YAML frontmatter (Format A). For each non-compliant issue:

1. Flag it to the user with current body format
2. Offer to add frontmatter derived from:
   - `spec_id` from title
   - `version` from labels
   - `status` from issue state + labels
   - `depends_on` from body text or `blockedBy` relationships
3. If approved, prepend frontmatter and update via `gh issue edit --body-file`

### Step 8: Report Summary

Summarize:
- Issues created (with links)
- Roadmap entries added
- Conflicts resolved (and how)
- Dependencies added/removed
- Labels fixed
- Bodies normalized

### Step 9: Advise Next Steps

- If issues were created: "Run `/shape-spec` on new issues to flesh out acceptance criteria."
- If roadmap entries were added: "Review the roadmap additions and adjust descriptions as needed."
- If dependencies changed: "Verify dependency graph looks correct on GitHub."
- If label hygiene issues remain: "Review flagged issues that need manual attention."

### Step 10: Exit

Print one of these exit signals (machine-readable):

- `sync-roadmap-status: synced` — all sources aligned, no conflicts remain
- `sync-roadmap-status: conflicts` — unresolvable conflicts flagged for manual review
- `sync-roadmap-status: error` — prerequisites missing or unrecoverable issue

## Does NOT

- Create or modify spec.md files (that's shape-spec/do-task)
- Run tests or verification gates
- Make sync decisions autonomously — the user approves everything
- Assign specs to milestones or projects
- Close or reopen issues (only syncs labels and metadata)
- Push commits (only modifies roadmap.md locally)

## Workflow

```
roadmap.md ←→ /sync-roadmap ←→ GitHub issues (type:spec)
                    ↓
         /shape-spec (for new issues)
```

This command keeps the roadmap and GitHub issues in sync, ensuring consistent spec tracking across both surfaces.
