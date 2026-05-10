---
schema: tanren.behavior.v0
id: B-0135
title: Uninstall Tanren assets without deleting user work
area: project-setup
personas: [solo-builder, team-builder, operator]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: asserted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can uninstall Tanren-managed assets from a repository so the repository can stop using Tanren without deleting user work.

## Preconditions

- The repository has Tanren assets installed.
- The user has permission to modify repository support files.

## Observable outcomes

- Generated Tanren assets can be removed deliberately.
- User-owned files, specs, and source signals are preserved unless explicitly exported or removed by separate action.
- The uninstall preview makes destructive effects visible before confirmation.

## Out of scope

- Deleting hosted account or project history.
- Removing external tracker issues or pull requests.

## Related

- B-0068
- B-0069
- B-0063

## Evidence note

Executable BDD proof exists in
`tests/bdd/features/B-0135-uninstall-tanren-assets-without-deleting-user-work.feature`.
Positive and falsification witnesses cover the CLI interface: confirmed apply
removes generated assets and preserves user-owned files; preview-only runs
perform no writes; repos without a manifest report a clear no-op. The canonical
interface set (`cli`) and scope boundaries (no account/project history deletion)
are unchanged from the accepted behavior definition.
