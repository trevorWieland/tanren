---
schema: tanren.behavior.v0
id: B-0071
title: Use the repository's installed standards
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can rely on Tanren commands to use the standards
installed for the repository so that checks and guidance reflect the project's
chosen way of working. This behavior is witnessed through
`tanren-cli standards inspect`.

## Preconditions

- The repository has Tanren support files installed.
- The repository has a configured standards location in
  `.tanren/project-methodology.toml`.

## Observable outcomes

- `tanren-cli standards inspect` succeeds with
  `status=ok command=standards.inspect` output when configured standards are
  present.
- `tanren-cli standards inspect` fails explicitly with
  `error: standards_missing -` when the configured standards root is missing.
- `tanren-cli standards inspect` fails explicitly with
  `error: standards_parse_failed -` and
  `failed to parse standards frontmatter in '<path>': <reason>` when
  standards frontmatter parsing fails.
- Missing standards are not silently replaced with unrelated fallback content
  during command execution.

## Out of scope

- Remote standards registries.
- Organization-level standards sync.
- Performing local repository standards checks from phone-only interfaces.

## Related

- B-0049
- B-0068
