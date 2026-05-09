@B-0070
Feature: Generate selected agent integrations
  A builder can constrain `tanren-cli install` integration output without
  skipping required standards profile assets.

  Rule: CLI selected integrations and validation

    @positive @cli
    Scenario: Install with claude and codex integrations only
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo" and integrations "claude,codex"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And only integrations "claude,codex" command assets are installed
      And rust-cargo profile standards files are installed
      And repository file ".opencode/commands/plan-product.md" does not exist

    @falsification @cli
    Scenario: Install excludes unselected integration assets
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command succeeds
      And only integrations "codex" command assets are installed
      And repository file ".claude/commands/plan-product.md" does not exist
      And repository file ".opencode/commands/plan-product.md" does not exist

    @falsification @cli
    Scenario: Reject install when integrations input is invalid
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo" and integrations "claude,not-an-integration"
      Then the install command exits nonzero
      And the install output reports a validation failure
      And the install stderr contains "unsupported install integration 'not-an-integration'"
      And no files are written in the repository fixture
