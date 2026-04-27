# MCP Setup

This document describes the active stdio MCP setup for Tanren command
installation and agent tool access.

Current canonical MCP contract is stdio `tanren-mcp` configured by
`tanren-cli install` and secured via capability envelopes.

Use:
- [architecture/install-targets.md](architecture/install-targets.md)
- [architecture/agent-tool-surface.md](architecture/agent-tool-surface.md)
- [methodology/commands-install.md](methodology/commands-install.md)

Required secure startup chain:
- config in `tanren.yml` under `methodology.mcp.security`
- installed binaries discoverable on `PATH` (`tanren-cli`, `tanren-mcp`)
- runtime injection of `TANREN_MCP_CAPABILITY_ENVELOPE` per phase/session
