# Interfaces

Canonical list of interface IDs referenced by every behavior file. Every
behavior's `interfaces:` frontmatter field must use one or more IDs from this
document (or the literal `any` when a capability is surface-agnostic).

These IDs are identity handles for behavior cross-referencing. Detailed
interface contracts live elsewhere (see links below) — this file is
intentionally terse.

To add a new interface, add a row with a stable ID slug and a one-sentence
description. Do not rename IDs once they are in use.

---

| ID | Description |
|----|-------------|
| `cli` | The `tanren` command-line tool invoked by a human at a terminal. |
| `api` | The HTTP API used by scripts, CI/CD systems, and external programs. |
| `mcp` | The MCP server used by LLM-based agents (e.g. Claude Code) to invoke Tanren as tools. |
| `tui` | The terminal user interface for real-time interactive operation. |
| `daemon` | The long-running control-plane process (`tanrend`) that drives scheduled and event-triggered work without a human in the loop. |
| `any` | Special value, not an interface. Use only when a capability is identical across every interface above. |

## Related documents

- Legacy Python interfaces: `docs/interfaces.md`
- Rewrite crate topology and transport binaries: `docs/rewrite/CRATE_GUIDE.md`
