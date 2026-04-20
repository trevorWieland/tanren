# Tanren Clean-Room Rewrite Planning Docs

This folder contains forward-looking planning documents for the clean-room
tanren reimplementation.

These docs are intentionally separate from current implementation docs so we
can design the target architecture without conflating it with the existing
Python system.

## Documents

- `MOTIVATIONS.md` - why rewrite, current pain points, target vision
- `HLD.md` - high-level architecture, planes, subsystems, and key flows
- `ROADMAP.md` - phased delivery plan with lanes and exit criteria
- `DESIGN_PRINCIPLES.md` - decision rules for architecture and implementation
- `CONTAINER_SYSTEM.md` - execution lease lifecycle, container/runtime model, and security/policy boundaries
- `RUST_STACK.md` - recommended Rust toolchain, crate topology, and core library stack
- `CRATE_GUIDE.md` - proposed Rust workspace crate map, dependency graph, linking rules, and `just` workflow model

## Usage

Use this set as the planning baseline for:

- rewrite ADRs
- branch strategy
- workstream decomposition
- scope control during implementation
