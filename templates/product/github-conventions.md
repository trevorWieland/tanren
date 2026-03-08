# GitHub Issue Conventions

Canonical reference for GitHub issue format, labels, and dependency management across all tanren commands. All commands that create or update issues MUST follow these conventions.

## Issue Body Format (Format A)

Every `type:spec` issue body MUST start with YAML frontmatter:

```markdown
---
spec_id: sX.Y.ZZ
version: vX.Y
status: planned
depends_on: [sX.Y.ZZ, sX.Y.ZZ]
---

{description}
```

Fields:
- `spec_id` — unique identifier (see Resolving spec_id below)
- `version` — target version (must match the `version:vX.Y` label)
- `status` — `planned`, `in-progress`, `done`, `wontfix`
- `depends_on` — list of `spec_id`s this issue depends on (empty list `[]` if none)

## Issue Title Format

```
{spec_id} {short title}
```

Example: `s0.1.42 LLM Provider Abstraction`

## Labels

| Label | Meaning |
|-------|---------|
| `type:spec` | This issue tracks a spec |
| `status:planned` | Not yet started |
| `status:done` | Completed |
| `status:wontfix` | Cancelled / won't do |
| `version:vX.Y` | Target version |

## Resolving spec_id

Convention: `s{major}.{minor}.{sequential_two_digit}`

To allocate the next `spec_id` for a version:

1. Query GitHub issues for the target version:
   ```bash
   gh issue list --label "type:spec" --label "version:vX.Y" --state all --json number,title --limit 200
   ```
2. Parse `spec_id`s from issue titles (pattern: `sX.Y.ZZ`)
3. Take the highest sequential number and increment by 1, zero-padded to two digits

## Dependency Relationships (blockedBy)

GitHub's `blockedBy`/`blocking` relationship tracks dependencies natively. Use `addBlockedBy` (NOT `addSubIssue`, which is single-parent).

### Getting node IDs

GraphQL requires node IDs, not issue numbers. Query them in bulk:

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

Paginate if `hasNextPage` is true. Build an `issue_number → node_id` mapping.

### Adding a blockedBy relationship

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

Where `issueId` = the blocked issue (dependent) and `blockingIssueId` = the blocking issue (dependency).

### Removing a blockedBy relationship

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

> **Note:** The `addBlockedBy`/`removeBlockedBy` mutations require Sub-Issues (public preview). If unavailable, use YAML frontmatter `depends_on` as the authoritative source for dependency tracking.

### Querying blockedBy relationships

```bash
gh api graphql -f query='
{
  repository(owner: "{owner}", name: "{repo}") {
    issue(number: {N}) {
      id
      trackedInIssues(first: 10) {
        nodes { number title }
      }
      trackedIssues(first: 10) {
        nodes { number title }
      }
    }
  }
}'
```

- `trackedInIssues` = issues that block this one (this issue is blocked by them)
- `trackedIssues` = issues this one blocks (they are blocked by this issue)

## Updating Issue Bodies

When updating an existing issue body, ALWAYS preserve the YAML frontmatter. Update fields within the frontmatter as needed, then append or replace content below it.

```bash
gh issue edit {number} --body-file /tmp/issue_body.md
```

## Creating Issues — Quick Template

```bash
gh issue create \
  --title "{spec_id} {title}" \
  --label "type:spec" --label "status:planned" --label "version:{version}" \
  --milestone "{version}" \
  --body-file /tmp/issue_body.md
```

Where `/tmp/issue_body.md` contains the Format A body with YAML frontmatter.
