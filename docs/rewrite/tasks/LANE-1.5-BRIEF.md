# Lane 1.5 — Worker Runtime — Agent Brief

## Task

Ship the Phase 1 worker runtime and produce Phase 1 proof artifacts that
demonstrate durable execution, retry policy behavior, and safe recovery.

## Read first

1. [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md) (Feature 4, 5, 6)
2. [LANE-1.5-WORKER-RUNTIME.md](LANE-1.5-WORKER-RUNTIME.md)
3. [../DESIGN_PRINCIPLES.md](../DESIGN_PRINCIPLES.md)
4. [../CRATE_GUIDE.md](../CRATE_GUIDE.md)
5. [../../../CLAUDE.md](../../../CLAUDE.md)

## Deliverables

| Area | Deliverable |
|------|-------------|
| Runtime | Durable worker flow from claim to terminal outcome |
| Retries | Typed retry-policy execution with visible attempt history |
| Recovery | Restart/crash recovery behavior that avoids duplicate terminalization |
| Evidence | Reproducible Phase 1 proof-pack generation and verification path |

## Non-negotiables

1. Work reaches one coherent terminal outcome per accepted execution path.
2. Retry policy behavior is explicit, bounded, and observable.
3. Recovery behavior is deterministic and test-verified.
4. Phase 1 proof claims are reproducible by another engineer.

## Done when

1. Worker runtime covers success/failure/cancel/retry/recovery paths.
2. Retry and non-retry classifications behave per policy expectations.
3. Recovery falsification cases pass with no duplicate terminal side effects.
4. Phase 1 proof pack can be generated and verified from documented commands.
