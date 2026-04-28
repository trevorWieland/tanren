---
id: B-0241
title: Manage cloud or VM provider accounts
area: runtime-substrate
personas: [operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can manage cloud or VM provider accounts so Tanren can place execution on approved external infrastructure.

## Preconditions

- The user has permission to manage execution provider accounts.
- The provider supports a Tanren-compatible execution target.

## Observable outcomes

- Provider accounts show owner scope, allowed projects, placement capabilities, quota, health, and credential ownership.
- Placement policy determines which work may use each provider account.
- Removing or disabling an account shows affected execution targets and in-flight work.

## Out of scope

- Replacing provider-native infrastructure administration.
- Exposing cloud credentials or VM private keys.

## Related

- B-0081
- B-0108
- B-0230
