---
id: B-0234
title: Grant worker-scoped temporary access
area: runtime-substrate
personas: [operator]
runtime_actors: [agent-worker]
interfaces: [cli, api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or authorized automation can grant worker-scoped temporary access so execution receives only the access needed for a specific task.

## Preconditions

- Work is ready to run in an execution environment.
- Credential use policy permits temporary worker access for the requested scope.

## Observable outcomes

- Temporary access is tied to a specific worker, task, scope, expiration, and permission set.
- The worker can use the access only within the approved execution scope.
- Access grants are visible in audit and usage views without revealing secret values.

## Out of scope

- Granting long-lived user or service-account credentials to workers by default.
- Allowing temporary access to outlive its approved scope.

## Related

- B-0102
- B-0104
- B-0235
