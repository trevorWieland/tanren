@B-0068
Feature: Bootstrap Tanren assets into an existing repository
  A builder can run `tanren-cli install` to bootstrap Tanren assets for a
  supported profile, and invalid inputs fail before any repository writes.

  Rule: CLI install and reinstall outcomes

    @positive @cli
    Scenario: Install rust-cargo assets into a clean repository
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And rust-cargo defaults install all methodology command assets and standards files
      And the install manifest records the rust-cargo profile and default integrations

    @positive @cli
    Scenario: Reinstall rust-cargo assets reconciles generated and standards drift
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file ".codex/skills/plan-product.md" contains "stale generated codex command"
      And repository file ".codex/skills/plan-product.md" baseline is recorded
      And repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And previous install manifest tracks stale generated file ".codex/skills/retired-command.md"
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" baseline is recorded
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" is deleted from the repository fixture
      And repository file "profiles/rust-cargo/rust/type-safety-patterns.md" contains "team-edited standards content"
      And repository file "profiles/rust-cargo/rust/type-safety-patterns.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And repository file ".codex/skills/plan-product.md" is replaced from its baseline content
      And stale generated file ".codex/skills/retired-command.md" is removed
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" preserves its baseline content
      And repository file "profiles/rust-cargo/rust/type-safety-patterns.md" preserves its baseline content

    @falsification @cli
    Scenario: Reject install when profile input is invalid
      Given a clean repository fixture
      When tanren-cli install runs with profile "not-a-profile"
      Then the install command exits nonzero
      And the install output reports a validation failure
      And no files are written in the repository fixture

    @falsification @cli
    Scenario: Reinstall does not overwrite user-edited standards content
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "profiles/rust-cargo/global/dependency-management.md" contains "custom organization dependency policy"
      And repository file "profiles/rust-cargo/global/dependency-management.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content
