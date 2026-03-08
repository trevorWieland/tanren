# Prefer Dependency Updates

Prefer frequent dependency updates to stay on latest stable features and performance. Don't pin old versions.

```python
# ✓ Good: Update dependencies frequently
# pyproject.toml - stay on latest stable
[dependencies]
pydantic = ">=2.0,<3.0"  # Updates within 2.x, not pinned to 2.1.3
openai = "^2.11.0"  # Allows 2.11.x, 2.12.x, etc.

# Use uv regularly to update
$ uv sync --upgrade

# ✗ Bad: Pin to old versions
[dependencies]
pydantic = "==2.1.3"  # Stuck on old version, missing fixes/features
openai = "==2.0.0"  # Won't get security updates or new features
```

**Update strategy:**
- Use compatible version ranges (`>=2.0,<3.0` or `^2.11.0`) instead of exact pins
- Update dependencies regularly with `uv sync --upgrade`
- Review changelogs for breaking changes and migrate
- Stay within major version when possible, but don't fear major updates

**Dependency management:**
- `uv.lock` records exact versions for reproducibility
- CI validates lockfile is up-to-date
- Update lockfile regularly as part of workflow
- Pin only when necessary (compatibility constraints)

**Exceptions:**
Stay on older version only when new version has critical bugs that:
1. Block your workflow completely
2. Have no workarounds available
3. Are actively tracked with upstream issue

**Breaking changes are NOT exceptions:**
- Migrate through breaking changes
- Don't stay on old versions to avoid migration cost
- Plan and execute migrations as part of update process

**Security implications:**
- Frequent updates = faster security patch adoption
- Don't delay updates for "convenience"
- Monitor dependency advisories and update promptly

**Why:** Get security fixes and vulnerability patches promptly, stay on latest stable features and performance improvements, and avoid accumulating technical debt from outdated dependencies.
