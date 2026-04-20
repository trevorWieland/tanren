# Phase 0 Proof Closure Brief

## Purpose

Define the remaining work needed to:

1. close all Phase 0 proof gaps against `docs/rewrite/PHASE0_PROOF_BDD.md`,
2. document usage so another engineer can re-run the proof without tribal knowledge,
3. make proof collection fast, deterministic, and reviewable.

Date: 2026-04-20

---

## Current Snapshot

Phase 0 implementation coverage is strong (typed domain/state, replay, CLI+MCP tool surface,
capability enforcement, installer drift guards, auth/replay protections). The remaining work is
primarily **proof packaging** and **operator-facing reproducibility**, not core architecture.

---

## Remaining Gaps

| Gap ID | BDD Scope | Current State | Why It Is Still a Gap |
|---|---|---|---|
| G1 | All scenarios | Evidence exists across tests/docs/commands | No single scenario-to-artifact index with owners and rerun commands |
| G2 | All scenarios | `just ci` proves code health | No dedicated "Phase 0 proof run" that emits an evidence pack |
| G3 | Feature 8.1 manual walkthrough | Flow is specified in docs | No canonical walkthrough transcript/artifact bundle that demonstrates the whole story end-to-end |
| G4 | Feature 3.1/3.2 operator auth/replay proof | Tests exist | No operator-level proof script that demonstrates valid auth, invalid auth, and replay denial in one place |
| G5 | Feature 2.2/2.3 replay proof | Replay tests exist | No curated human-readable "source vs replayed state" and "invalid replay rollback" report pack |
| G6 | Project status narrative | Some docs still show lane 0.5 as in progress | Story drift reduces confidence for Phase 0 exit review |
| G7 | Token troubleshooting ergonomics | Verifier behavior is correct | Manual token minting is easy to get wrong (`exp - iat` math), causing false bug reports |

---

## Workstreams To Close Gaps

### W1. Phase 0 Evidence Index (G1)

Deliverables:
- `docs/rewrite/PHASE0_PROOF_EVIDENCE_INDEX.md`

Content requirements:
- one row per BDD scenario (1.1 through 8.1),
- positive witness artifact path,
- falsification witness artifact path,
- command(s) used to produce artifact,
- owner and freshness timestamp.

Acceptance:
- every scenario has both witness links,
- no "implied" proof; every claim points to an artifact.

### W2. Reproducible Proof Harness (G2, G4, G5)

Deliverables:
- `scripts/proof/phase0/run.sh` (collect evidence),
- `scripts/proof/phase0/verify.sh` (assert required files exist + check pass markers),
- `docs/rewrite/PHASE0_PROOF_RUNBOOK.md` (how to execute and read results).

Behavior requirements:
- output under `artifacts/phase0-proof/<timestamp>/`,
- stable directory structure by scenario ID,
- machine-readable summary (`summary.json`) and human-readable summary (`summary.md`).

Acceptance:
- fresh clone + documented prerequisites can produce a complete pack with one command,
- rerun is deterministic except for timestamps/ids.

### W3. Manual Walkthrough Proof Pack (G3)

Deliverables:
- `docs/rewrite/PHASE0_MANUAL_WALKTHROUGH.md`,
- one committed sample evidence bundle captured from a full 7-step loop.

Required proof contents:
- start state,
- each phase invocation (shape-spec -> walk-spec),
- resulting typed state transitions,
- task/finding/progress coherence across the full loop.

Acceptance:
- a technically literate non-Tanren specialist can follow the narrative and verify outcomes.

### W4. Replay Proof Packaging (G5)

Deliverables:
- replay parity report template in runbook,
- artifacts showing:
  - source event stream,
  - replayed state snapshot,
  - equivalence verdict,
  - invalid replay rejection with explicit diagnostics and no partial apply evidence.

Acceptance:
- Feature 2.2 and 2.3 are demonstrated without requiring code-level interpretation.

### W5. Canon Status Alignment (G6)

Deliverables:
- update stale Phase 0 status text in:
  - `docs/rewrite/ORIENTATION.md`
  - `docs/rewrite/tasks/README.md`

Acceptance:
- all "Current Position / Current Status" sections tell one consistent Phase 0 story.

### W6. Token Proof Utility + Guidance (G4, G7)

Deliverables:
- `scripts/proof/phase0/mint_actor_token.py` (or equivalent) with explicit modes:
  - valid,
  - wrong_issuer,
  - wrong_audience,
  - expired,
  - ttl_over_max,
  - replay_reuse fixture support,
- short troubleshooting doc section in runbook.

Required invariant checks in utility:
- prints computed `iat`, `exp`, `exp_minus_iat`,
- warns/fails when `exp - iat > actor_token_max_ttl_secs`.

Acceptance:
- invalid-token demos fail for intended reasons only,
- no accidental TTL math mistakes during proof runs.

---

## Definition Of Done (Phase 0 Proof Exit)

Phase 0 proof is complete when all conditions below are true:

1. Every BDD scenario has both positive and falsification artifacts.
2. Artifacts can be regenerated from documented commands on demand.
3. Manual walkthrough evidence is present and understandable to non-specialists.
4. Auth/replay proof includes explicit valid token and invalid-token classes.
5. Replay proof includes both equivalence and safe-failure/rollback evidence.
6. Canon status docs are internally consistent.
7. Proof harness returns non-zero on missing/invalid evidence.

---

## Invalid Token Investigation (Current Conclusion)

Conclusion: **implementation is correct; observed issue is test/setup token construction**.

Verified behavior:
- verifier enforces `0 < (exp - iat) <= max_ttl`,
- with default `max_ttl=900`:
  - token with `exp - iat = 905` is rejected,
  - token with `exp - iat = 900` is accepted.

Observed internal diagnostic:
- `actor_token_ttl_violation` with `exp`, `iat`, and `max_ttl`.

Operational rule for proof scripts:
- always compute `exp` from `iat`, not from wall clock:
  - `exp = iat + requested_ttl`,
  - require `requested_ttl <= actor_token_max_ttl_secs`.

---

## Suggested Execution Order

1. W1 (evidence index) and W5 (status alignment)
2. W2 (proof harness skeleton)
3. W6 (token utility; remove auth proof flakiness)
4. W4 (replay packaging)
5. W3 (manual walkthrough capture)
6. Final exit review against `PHASE0_PROOF_BDD.md`
