@B-0069
Feature: Detect installer drift without mutating files
  A builder can run `tanren-cli drift` against an installed repository to
  diagnose projection drift without modifying repository files.

  Rule: CLI drift diagnostics and non-mutating behavior

    @positive @cli
    Scenario: Drift check reports no drift for a clean installed repository
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      When tanren-cli drift runs with profile "rust-cargo"
      Then the drift command succeeds
      And the drift output reports no drift

    @positive @cli
    Scenario: Drift check reports generated asset drift
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file ".codex/skills/plan-product.md" contains "generated drift"
      When tanren-cli drift runs with profile "rust-cargo"
      Then the drift command exits nonzero
      And the drift output reports generated asset drift for ".codex/skills/plan-product.md"

    @positive @cli
    Scenario: Drift check reports missing preserved standards
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "profiles/rust-cargo/testing/mock-boundaries.md" is deleted from the repository fixture
      When tanren-cli drift runs with profile "rust-cargo"
      Then the drift command exits nonzero
      And the drift output reports missing preserved standard "profiles/rust-cargo/testing/mock-boundaries.md"

    @positive @cli
    Scenario: Drift check reports accepted preserved edits without failing
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "profiles/rust-cargo/global/dependency-management.md" contains "team-edited standards content"
      When tanren-cli drift runs with profile "rust-cargo"
      Then the drift command succeeds
      And the drift output reports accepted preserved edit "profiles/rust-cargo/global/dependency-management.md"

    @falsification @cli
    Scenario: Drift check does not mutate repository files
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "profiles/rust-cargo/global/dependency-management.md" baseline is recorded
      When tanren-cli drift runs with profile "rust-cargo"
      Then the drift command succeeds
      And no files are written in the repository fixture
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content

    @falsification @cli
    Scenario: Missing generated assets are reported as drift
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "profiles/rust-cargo/global/dependency-management.md" baseline is recorded
      And repository file ".codex/skills/plan-product.md" is deleted from the repository fixture
      When tanren-cli drift runs with profile "rust-cargo"
      Then the drift command exits nonzero
      And the drift output reports missing generated asset ".codex/skills/plan-product.md"
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content
