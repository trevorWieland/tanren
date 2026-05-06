---
schema: tanren.behavior.v0
id: B-0290
title: Reject corrupted invitation permissions on acceptance
area: governance
personas: [operator]
interfaces: [cli]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

When an invitation row in the store contains an org_permissions value that
fails domain validation (for example, a whitespace-only or otherwise
corrupted string), the invitation consumption path must surface a
store-layer data-invariant error rather than silently dropping the value
to `None`. Silent drop-through would grant the membership fewer
permissions than the invitation specified without any audible signal.

## Preconditions

- An invitation exists in the store with a corrupted org_permissions
  column value (a string that `OrgPermissions::parse` rejects).

## Observable outcomes

- Existing-account join (`accept_existing_invitation_atomic`) rejects
  the acceptance with a store error instead of creating a membership
  with `None` permissions.
- The legacy standalone `consume_invitation` path propagates the same
  data-invariant error instead of returning `None` for org_permissions.
- Valid org_permissions values continue to populate membership
  permissions correctly.

## Out of scope

- How the corrupted value entered the store (direct SQL, migration
  error, or older-version write).
- Invitation creation validation (R-0005).
- Project-level access changes (M-0031).

## Related

- B-0043
- B-0045
