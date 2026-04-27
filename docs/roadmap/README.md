# Tanren Roadmap

This directory is the planning canon for the Rust Tanren codebase.

Tanren is a Rust control plane for software delivery agents. It owns typed
work state, durable event history, command installation, proof artifacts, and
runtime orchestration boundaries. Planning documents here describe product
capabilities to build, not migration work.

## Documents

- [ROADMAP.md](ROADMAP.md) - phased product roadmap and exit criteria
- [phases/phase-1-runtime-substrate.md](phases/phase-1-runtime-substrate.md) - runtime substrate work
- [phases/phase-2-planner-orchestration.md](phases/phase-2-planner-orchestration.md) - planner-native orchestration work
- [../behaviors/README.md](../behaviors/README.md) - product behavior catalog
- [../../tests/bdd/README.md](../../tests/bdd/README.md) - executable behavior evidence rules

## Planning Rules

- Roadmap items are expressed as product capabilities.
- Phase documents describe desired behavior, interfaces, and acceptance
  evidence.
- Implementation notes belong near the Rust code or in focused architecture
  docs under `docs/architecture/`.
- `just ci` is the merge gate for roadmap work.
