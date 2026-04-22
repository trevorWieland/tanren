# Bootstrap Guide (Legacy / Archived)

This page documents the pre-rewrite Python-era bootstrap path and is kept only
for historical reference.

For Phase 0 acceptance and current operator workflow, use:
- [../methodology/commands-install.md](../methodology/commands-install.md)
- [../architecture/install-targets.md](../architecture/install-targets.md)
- [../rewrite/PHASE0_PROOF_RUNBOOK.md](../rewrite/PHASE0_PROOF_RUNBOOK.md)

Canonical runtime contract is installed binaries:
- `tanren-cli`
- `tanren-mcp`

Use:

```bash
scripts/runtime/install-runtime.sh
scripts/runtime/verify-installed-runtime.sh
tanren-cli install
```

Do not treat the old `scripts/install.sh --profile python-uv` path on this page
as an acceptance path for the rewrite lane.
