@B-0070
Feature: Generate selected agent integrations deterministically
  A solo-builder or team-builder can restrict Tanren installation to selected
  agent integrations so that the repository only receives the integration
  assets the user requested. Standards required by the chosen profile are still
  installed.

  Background:
    Given a temporary repository

  @positive @cli
  Scenario: Selected integrations receive their assets and standards are still installed
    When the installer runs with profile "rust-cargo" and integrations "claude"
    Then the installer succeeds
    And the file ".claude/commands/project/identify-behaviors.md" exists in the repository
    And the file "standards/default/global/tech-stack.md" exists in the repository
    And the file "standards/rust-cargo/rust/naming-conventions.md" exists in the repository
    And the file ".tanren/install-manifest.json" exists in the repository

  @falsification @cli
  Scenario: Unselected integrations are not written
    When the installer runs with profile "rust-cargo" and integrations "claude"
    Then the installer succeeds
    And the file ".codex/skills/project/identify-behaviors.md" does not exist in the repository
    And the file ".opencode/commands/project/identify-behaviors.md" does not exist in the repository

  @falsification @cli
  Scenario: Invalid integration name is rejected before any files are written
    When the installer runs with profile "rust-cargo" and integrations "claude,nonexistent"
    Then the installer fails with message containing "unknown integration"
    And the file ".tanren/install-manifest.json" does not exist in the repository
