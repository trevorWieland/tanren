# Triage Audits

Load standards audit reports, analyze violations across all standards, group findings by root cause, and create GitHub issues for the most impactful fixes. Interactive — the user approves the triage before any issues are created.

**Suggested model:** Strong reasoner with good judgment (e.g., Opus via TUI). Grouping violations by root cause requires cross-standard analysis and the user must approve the triage.

## Important Guidelines

- Human-in-the-loop — the user approves the triage before any issues are created
- Always use AskUserQuestion tool when asking the user anything. In TUI: use AskUserQuestion. In Discord/NanoClaw: use send_message with numbered options and wait for reply.
- Group by root cause / natural fix scope, not per-standard
- Priority is a function of score, importance, and violation count
- Every created issue must be actionable with clear scope

## When to Use

- After running `./tanren/scripts/audit-standards.sh`
- When audit reports exist in `tanren/audits/{date}/`
- Periodically to check standards drift

## Prerequisites

1. Audit reports exist in `tanren/audits/{date}/` (from audit-standards.sh)
2. Standards index exists at `tanren/standards/index.yml`
3. `gh` CLI is authenticated for creating issues

If no audit reports are found, ask the user which date folder to use or whether to run the audit script first.

## Process

### Step 1: Locate Audit Reports

Find the most recent audit run:

1. List directories in `tanren/audits/` sorted by name (date format ensures chronological order)
2. Use the most recent `YYYY-MM-DD` directory, or a specific date if provided by the user
3. Verify it contains `.md` files (excluding the template)

If no reports found, exit with `triage-audits-status: no-reports`.

### Step 2: Parse All Reports

For each `.md` file in the audit directory:

1. Parse the YAML frontmatter to extract:
   - `standard`: slug identifier
   - `category`: standard category
   - `score`: 0-100 compliance score
   - `importance`: Critical/High/Medium/Low
   - `violations_count`: number of violations
   - `status`: clean or violations-found
   - `date`: audit date

2. Parse the violations section to extract each violation:
   - Short description
   - File:line reference
   - Severity
   - Evidence (code snippet)
   - Recommendation

3. Build a structured dataset of all standards and their violations.

### Step 3: Score and Prioritize

Assign importance weights:
- Critical = 4
- High = 3
- Medium = 2
- Low = 1

Calculate priority score for each standard:
```
priority = (100 - score) * importance_weight
```

Sort standards by priority (highest first).

Produce an overview table:

```
| Standard                  | Category     | Score | Importance | Violations | Priority |
|---------------------------|--------------|-------|------------|------------|----------|
| strict-typing-enforcement | python       | 45    | High       | 23         | 165      |
| async-first-design        | python       | 52    | Critical   | 18         | 192      |
| ...                       | ...          | ...   | ...        | ...        | ...      |
```

### Step 4: Group Violations by Root Cause

This is the critical step. Do NOT create one issue per standard. Instead, analyze all violations across all standards and group them by **natural fix scope** — work that would logically be done together in one PR.

Grouping heuristics:

1. **Same files, related standards:** If `modern-python-314` and `strict-typing-enforcement` both have violations in the same files, group them as "modernize type annotations in {module}".

2. **Same architectural layer:** If `async-first-design` violations are concentrated in the storage layer, group them as "convert storage adapters to async".

3. **Same test infrastructure:** If `no-test-skipping`, `mandatory-coverage`, and `three-tier-test-structure` violations overlap in the same test directories, group them as "fix test infrastructure in {area}".

4. **Same root cause:** If violations across multiple standards stem from the same underlying issue (e.g., "these files were written before standard X existed"), group them together.

5. **Clean standards:** Standards with score >= 90 and status "clean" get no issues — just a note in the summary.

For each group, produce:
- **Title:** descriptive, action-oriented (e.g., "Modernize type annotations in core schemas")
- **Standards involved:** which standards this addresses
- **Files affected:** list of files with violation counts
- **Estimated scope:** Small (1-3 files), Medium (4-10 files), Large (10+ files)
- **Violations resolved:** how many total violations this group addresses

### Step 5: Present Triage to User

Present the analysis in this order:

1. **Score Distribution** — overview table from Step 3
2. **Clean Standards** — standards with no issues (celebrate what's working)
3. **Proposed Issue Groups** — from Step 4, ordered by priority

For each proposed issue group:

```
### Group N: {Title}
**Priority:** {High|Medium|Low}
**Standards:** {list of standard slugs}
**Scope:** {Small|Medium|Large} ({N} files, {N} violations)
**Files:**
- `{file}` — {N} violations ({standard1}, {standard2})
- ...

**Proposed issue title:** {title}
**Proposed labels:** type:spec, status:planned, {category labels}
```

Use AskUserQuestion to let the user:
- Approve or skip each issue group
- Adjust titles, labels, or scope
- Split or merge groups
- Set the target version label (e.g., `version:v0.2`)

### Step 6: Create GitHub Issues

For each approved group:

1. Determine the next available `spec_id` (see `tanren/product/github-conventions.md` → Resolving spec_id).
2. Create an issue with YAML frontmatter body (Format A):

```
gh issue create \
  --title "{spec_id} {title}" \
  --label "type:spec" --label "status:planned" --label "version:{version}" \
  --body "$(cat <<'EOF'
---
spec_id: sX.Y.ZZ
version: vX.Y
status: planned
depends_on: []
---

## Standards Compliance: {title}

**Source:** Standards audit ({date})
**Standards:** {list of standards with scores}
**Scope:** {N} files, {N} violations

## Violations

{For each file: file path, violations with evidence from audit reports}

## Acceptance Criteria

- [ ] All listed violations are resolved
- [ ] `make all` passes
- [ ] Re-audit of affected standards shows improved scores

## References

- Audit reports: `{audit_dir}/`
- Standards: {links to standard files}
EOF
)"
```

3. If the issue has dependencies on other specs, add `blockedBy` relationships via GraphQL (see `tanren/product/github-conventions.md` → Dependency Relationships).
4. Record each created issue number.

### Step 7: Write Triage Summary

Write `tanren/audits/{date}/TRIAGE.md`:

```markdown
# Standards Audit Triage — {date}

## Score Distribution

| Standard | Category | Score | Importance | Status |
|----------|----------|-------|------------|--------|
| ...      | ...      | ...   | ...        | ...    |

**Average score:** {N}/100
**Clean standards:** {N}/{total}
**Standards with violations:** {N}/{total}

## Issues Created

| # | Title | Standards | Scope | Priority |
|---|-------|-----------|-------|----------|
| #{N} | {title} | {standards} | {scope} | {priority} |

## Clean Standards (No Action Needed)

{list of standards with score >= 90}

## Deferred / Skipped

{any groups the user chose not to create issues for, with rationale}
```

### Step 8: Commit

Commit the triage summary:

```
git add tanren/audits/{date}/TRIAGE.md
git commit -m "Triage standards audit ({date}): {N} issues created"
```

### Step 9: Advise Next Steps

Tell the user what to do next:

- "Created {N} issues for standards compliance work."
- If high-priority issues exist: "Consider running `/shape-spec` on the top-priority issues first."
- "Re-run `./tanren/scripts/audit-standards.sh` after fixes to track improvement."
- Link to the created issues.

### Step 10: Exit

Print one of these exit signals (machine-readable):

- `triage-audits-status: complete` — triage done, issues created
- `triage-audits-status: no-reports` — no audit reports found
- `triage-audits-status: clean` — all standards passing, no issues needed
- `triage-audits-status: error` — prerequisites missing or unrecoverable issue

## Does NOT

- Run the audit itself (that's audit-standards.sh)
- Fix code (creates issues for shape-spec/do-task)
- Create issues without user approval
- Modify standards files
- Run tests or verification gates

## Workflow

```
audit-standards.sh → /triage-audits → gh issues → /shape-spec → orchestrator → PR
```

This command bridges the gap between automated standards auditing and the spec-based implementation workflow.
