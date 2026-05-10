---
schema: tanren.behavior.v0
id: B-0041
title: Bound active-account read models
area: governance
personas: [solo-builder, team-builder, observer, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

The active-account list and switch read path is bounded so that a caller
with many signed-in sessions cannot cause unbounded memory or CPU use on
the server or client. Window context is resolved once per request
lifecycle and reused until explicit rotation.

## Preconditions

- The caller has at least one signed-in account.

## Observable outcomes

- Server-side context construction rejects signed-in sets exceeding the
  documented limit (16 accounts).
- Server-side context construction rejects window maps exceeding the
  documented limit (16 windows).
- Generated web validators reject active-account responses with more
  than 16 entries.
- The web switcher uses bounded keyed lookup helpers rather than
  unbounded array scans.
- Window context is computed once per request lifecycle and cached until
  explicit rotation (sign-out or server rejection).

## Out of scope

- Account creation or invitation semantics.
- Per-window active-account switching (covered by B-0046).

## Related

- B-0043
- B-0046
