@behavior @installer @cli
Feature: Bootstrap Tanren assets into an existing repository

  @B-0068 @positive
  Scenario: Fresh bootstrap installs the default Tanren asset set
    Given an empty target repository
    When install is run with profile "rust-cargo"
    Then bootstrap writes tanren config, commands, MCP configs, and rust standards
    And generated config records profile "rust-cargo" and all default agents

  @B-0068 @positive
  Scenario: Reinstall converges generated commands while preserving standards
    Given a bootstrapped rust-cargo repository
    And a rendered command, unmanaged command file, edited standard, and missing standard
    When install is run again
    Then rendered commands are exhaustively replaced
    And edited standards are preserved while missing standards are restored

  @B-0068 @falsification
  Scenario: Invalid profile input fails before writing bootstrap files
    Given an empty target repository
    When install is run with invalid profile "../rust-cargo"
    Then install fails validation
    And no bootstrap files are written

  @B-0068 @falsification
  Scenario: Unknown profile input fails before writing bootstrap files
    Given an empty target repository
    When install is run with invalid profile "missing-profile"
    Then install fails validation
    And no bootstrap files are written

  @B-0068 @falsification
  Scenario: Conflicting bootstrap flags are rejected for an existing config
    Given a bootstrapped rust-cargo repository
    When install is run with profile "react-ts-pnpm"
    Then install fails validation

  @B-0068 @falsification
  Scenario: Legacy config shapes are rejected
    Given a target repository with legacy methodology profiles
    When install is run without bootstrap flags
    Then install fails validation

  @B-0068 @falsification
  Scenario: Malformed MCP config fails without partial command rewrites
    Given a bootstrapped rust-cargo repository
    And an existing MCP config is malformed
    When install is run again
    Then install fails validation
    And rendered commands are not rewritten after the MCP failure

