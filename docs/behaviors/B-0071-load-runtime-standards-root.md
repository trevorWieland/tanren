---
id: B-0071
title: Load runtime standards from the installed standards root
personas: [solo-dev, team-dev]
interfaces: [cli]
contexts: [personal, organizational]
status: accepted
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can rely on methodology commands to load standards
from the configured runtime standards root so that command behavior reflects
the repository's installed standards.

## Preconditions

- The repository has a Tanren methodology config that points at installed
  standards.

## Observable outcomes

- Runtime standards loading succeeds when the configured standards root exists.
- Runtime standards loading fails explicitly when the configured standards root
  is missing.
- Missing standards are not silently replaced with unrelated fallback content
  during command execution.

## Out of scope

- Remote standards registries or organization-level standards sync.

## Related

- B-0049
- B-0068

