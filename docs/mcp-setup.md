# MCP Setup (Legacy / Archived)

This document describes the pre-rewrite SSE MCP surface and is not the active
Phase 0 acceptance path.

Current canonical MCP contract is stdio `tanren-mcp` configured by
`tanren-cli install` and secured via capability envelopes.

Use:
- [architecture/install-targets.md](architecture/install-targets.md)
- [architecture/agent-tool-surface.md](architecture/agent-tool-surface.md)
- [rewrite/PHASE0_PROOF_RUNBOOK.md](rewrite/PHASE0_PROOF_RUNBOOK.md)

Required secure startup chain for canonical runtime:
- config in `tanren.yml` under `methodology.mcp.security`
- installed binaries discoverable on `PATH` (`tanren-cli`, `tanren-mcp`)
- runtime injection of `TANREN_MCP_CAPABILITY_ENVELOPE` per phase/session
