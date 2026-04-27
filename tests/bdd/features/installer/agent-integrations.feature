@behavior @installer @cli
Feature: Generate selected agent integrations deterministically

  @B-0070 @positive
  Scenario: Bootstrap can restrict agent integrations
    Given an empty target repository
    When install is run with profile "rust-cargo" and agents "codex"
    Then bootstrap writes only the codex command and MCP config targets

  @B-0070 @falsification
  Scenario: Unknown agent integration is rejected before bootstrap
    Given an empty target repository
    When install is run with profile "rust-cargo" and agents "missing-agent"
    Then install fails validation
    And no bootstrap files are written

