@B-0070
Feature: Select agent integrations during Tanren install
  A builder can choose which integration assets are installed while Tanren
  still installs required standards assets for the selected profile.

  Rule: CLI selected integrations and validation

    @positive @cli
    Scenario: Install only selected integration assets
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo" and integrations "claude,codex"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And only integrations "claude,codex" command assets are installed
      And repository file ".opencode/commands/plan-product.md" does not exist

    @positive @cli
    Scenario: Install still writes required standards with selected integrations
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command succeeds
      And rust-cargo profile standards files are installed

    @falsification @cli
    Scenario: Install excludes unselected integration assets
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command succeeds
      And only integrations "codex" command assets are installed
      And repository file ".claude/commands/plan-product.md" does not exist
      And repository file ".opencode/commands/plan-product.md" does not exist

    @falsification @cli
    Scenario: Reject install when integrations input includes an invalid name
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo" and integrations "claude,not-an-integration"
      Then the install command exits nonzero
      And the install output reports a validation failure
      And the install stderr contains "unsupported install integration 'not-an-integration'"
      And no files are written in the repository fixture
