---
schema: tanren.behavior.v0
id: B-0134
title: Upgrade installed Tanren assets
area: project-setup
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: implemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can upgrade Tanren assets installed in a repository so command and standards support can move forward deliberately.

## Preconditions

- The repository has Tanren assets installed.
- The user has permission to modify repository support files.

## Observable outcomes

- The user can preview the upgrade before applying it.
- The preview reports compatibility concerns including destructive-asset-changes and no-install-manifest.
- The preview reports specific migration-concern paths before any destructive action.
- The upgrade does not proceed past preview without explicit confirmation.
- Confirmed apply updates generated assets and writes an updated install manifest.
- Confirmed apply preserves user-owned standards files.
- Confirmed apply reports noop when no install manifest is present.
- Preview and apply on the same fixture correlate through a deterministic preview identifier derived from the changed path set, preventing apply from passing using an unrelated preview.
- The preview and apply outputs both include the preview_id field so that correlation is observable across all interface witnesses.
- The upgraded install manifest records the current manifest version.
- Legacy migration-concern paths are removed after confirmed apply.
- Stale manifest entries for migrated paths are no longer present in the updated manifest.

## Verification evidence status

- Implemented behavior proof: `tests/bdd/features/B-0134-upgrade-installed-tanren-assets.feature`.
- Proof covers per-interface positive and falsification witnesses across web, api, mcp, cli, and tui.
- Each interface witness exercises the CLI command adapter with its corresponding harness dispatch.
- Positive witnesses cover: preview with migration concern reporting, confirmed apply with generated asset replacement, user-owned file preservation, manifest version assertion, migrated manifest entry removal, destructive concern surfacing, preview-apply path correlation, and preview-apply preview_id correlation.
- Falsification witnesses cover: no-install-manifest concern on empty repository, confirmed apply noop on missing manifest, no-confirm preview preserving user-owned files, and missing-install apply remaining noop after preview with preview_id correlation.
- Preview-apply correlation is enforced by a deterministic `PreviewId` derived from the sorted changed-path set. Both preview and apply output lines include the `preview_id` field. BDD witnesses assert that the apply preview_id matches the preview preview_id, ensuring apply cannot pass using an unrelated preview.

## Out of scope

- Upgrading external agent tools.
- Silently overwriting user-owned work.

## Related

- B-0068
- B-0069
- B-0070
