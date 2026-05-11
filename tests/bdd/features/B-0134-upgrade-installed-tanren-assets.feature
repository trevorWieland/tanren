@B-0134
Feature: Upgrade installed Tanren assets
  A builder can upgrade Tanren assets in an installed repository, previewing
  changes before apply and confirming that generated assets update while
  user-owned files remain untouched.

  Rule: CLI upgrade preview, confirm, and apply outcomes

    @positive @cli
    Scenario: Upgrade replaces stale generated assets and restores missing standards
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file ".codex/skills/plan-product.md" contains "stale generated codex command"
      And repository file ".codex/skills/plan-product.md" baseline is recorded
      And repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And previous install manifest tracks stale generated file ".codex/skills/retired-command.md"
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" baseline is recorded
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" is deleted from the repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And repository file ".codex/skills/plan-product.md" is replaced from its baseline content
      And stale generated file ".codex/skills/retired-command.md" is removed
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" preserves its baseline content

    @positive @cli
    Scenario: Confirmed apply produces exactly the preview writes and removals
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file ".codex/skills/plan-product.md" contains "stale generated codex command"
      And repository file ".codex/skills/plan-product.md" baseline is recorded
      And repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And previous install manifest tracks stale generated file ".codex/skills/retired-command.md"
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And repository file ".codex/skills/plan-product.md" is replaced from its baseline content
      And stale generated file ".codex/skills/retired-command.md" is removed

    @positive @cli
    Scenario: Upgrade reports migration concerns for stale generated assets
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And previous install manifest tracks stale generated file ".codex/skills/retired-command.md"
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And stale generated file ".codex/skills/retired-command.md" is removed

    @falsification @cli
    Scenario: Upgrade does not overwrite user-edited standards content
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "profiles/rust-cargo/global/dependency-management.md" contains "custom organization dependency policy"
      And repository file "profiles/rust-cargo/global/dependency-management.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content

    @falsification @cli
    Scenario: Upgrade does not remove unrelated files from crafted stale manifest entries
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "README.md" contains "repository-owned content"
      And repository file "README.md" baseline is recorded
      And previous install manifest tracks stale generated file "README.md"
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And repository file "README.md" preserves its baseline content

    @falsification @cli
    Scenario: Upgrade preserves stale generated file when content drifted after manifest hash
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And previous install manifest tracks stale generated file ".codex/skills/retired-command.md"
      And repository file ".codex/skills/retired-command.md" contains "team-edited stale generated command"
      And repository file ".codex/skills/retired-command.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And repository file ".codex/skills/retired-command.md" preserves its baseline content

    @falsification @cli
    Scenario: Reject upgrade when manifest is tampered with traversal stale path
      Given a clean repository fixture
      And repository file "README.md" contains "repository-owned content"
      And repository file "README.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given previous install manifest is tampered with raw generated path "../README.md"
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command exits nonzero
      And the install output reports a validation failure
      And no files are written in the repository fixture
      And repository file "README.md" preserves its baseline content

    @falsification @cli
    Scenario: Reject upgrade when manifest is tampered with malformed content hash
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given previous install manifest is tampered with an invalid content hash entry
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command exits nonzero
      And the install output reports a validation failure
      And no files are written in the repository fixture

    @falsification @cli
    Scenario: Reject upgrade atomically when apply validation fails after stale removals are planned
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command succeeds
      Given repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And previous install manifest tracks stale generated file ".codex/skills/retired-command.md"
      And repository file ".codex/skills/retired-command.md" baseline is recorded
      And repository file "outside-command.md" contains "external command file"
      And repository path ".codex/skills/plan-product.md" is replaced with a file symlink to fixture path "outside-command.md"
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command exits nonzero
      And the install output reports a validation failure
      And no files are written in the repository fixture
      And repository file ".codex/skills/retired-command.md" preserves its baseline content
      And repository file "outside-command.md" contains "external command file"
