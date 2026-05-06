@B-0068
Feature: Bootstrap Tanren assets into an existing repository
  A solo-builder or team-builder can bootstrap Tanren methodology assets
  (commands, agent integrations, standards, and install manifest) into an
  existing repository by choosing a supported standards profile. Re-running
  install replaces generated assets, removes stale generated assets,
  preserves user-edited standards, and restores missing standards. Invalid
  bootstrap input is rejected before any assets are written.

  Background:
    Given a temporary repository

  @positive @cli
  Scenario: Fresh install with a valid profile creates commands, all integrations, standards, and manifest
    When the installer runs with profile "rust-cargo"
    Then the installer succeeds
    And the file "commands/project/identify-behaviors.md" exists in the repository
    And the file ".claude/commands/project/identify-behaviors.md" exists in the repository
    And the file ".codex/skills/project/identify-behaviors.md" exists in the repository
    And the file ".opencode/commands/project/identify-behaviors.md" exists in the repository
    And the file "standards/default/global/tech-stack.md" exists in the repository
    And the file "standards/rust-cargo/rust/naming-conventions.md" exists in the repository
    And the file ".tanren/install-manifest.json" exists in the repository

  @positive @cli
  Scenario: Reinstall replaces generated assets, removes stale generated assets, preserves user-edited standards, and restores missing standards
    When the installer runs with profile "rust-cargo" and integrations "claude,codex,opencode"
    Then the installer succeeds
    And the file ".codex/skills/project/identify-behaviors.md" exists in the repository
    When the file "standards/default/global/tech-stack.md" is modified to contain
      """
      preserved-user-content-marker
      """
    When the file "standards/rust-cargo/rust/naming-conventions.md" is deleted
    When the installer runs with profile "rust-cargo" and integrations "claude"
    Then the installer succeeds
    And the file "standards/default/global/tech-stack.md" in the repository contains
      """
      preserved-user-content-marker
      """
    And the file "standards/rust-cargo/rust/naming-conventions.md" exists in the repository
    And the file ".claude/commands/project/identify-behaviors.md" exists in the repository
    And the file ".codex/skills/project/identify-behaviors.md" does not exist in the repository

  @falsification @cli
  Scenario: Invalid profile is rejected before any files are written
    When the installer runs with profile "nonexistent"
    Then the installer fails with message containing "unknown profile"
    And the file ".tanren/install-manifest.json" does not exist in the repository

  @falsification @cli
  Scenario: Reinstall does not overwrite user-edited standards content
    When the installer runs with profile "rust-cargo"
    Then the installer succeeds
    When the file "standards/default/global/tech-stack.md" is modified to contain
      """
      my-custom-tech-choice-unique-marker
      """
    When the installer runs with profile "rust-cargo"
    Then the installer succeeds
    And the file "standards/default/global/tech-stack.md" in the repository contains
      """
      my-custom-tech-choice-unique-marker
      """
