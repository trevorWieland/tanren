@B-0134
Feature: Upgrade installed Tanren assets
  A solo-builder or team-builder can upgrade Tanren assets installed
  in a repository. The upgrade preview shows what will change,
  reports compatibility concerns, and preserves user-owned files.
  Each interface in B-0134 (`web`, `api`, `mcp`, `cli`, `tui`) covers
  the same two witnesses: one positive (preview → confirm → assets
  updated, user files preserved) and one falsification (preview-only
  leaves the repository unchanged).

  Background:
    Given a repository with Tanren assets installed at version "0.1.0-fixture"

  Rule: API surface

    @positive @api
    Scenario: Confirmed upgrade over the API updates generated assets and preserves user files
      When the user previews the upgrade
      Then the preview includes actions to create, update, and remove generated assets
      And the preview reports migration concerns
      And the preview lists user-owned paths as preserved
      When the user confirms and applies the upgrade
      Then generated assets are updated to the target version
      And user-owned files are unchanged

    @falsification @api
    Scenario: Upgrade preview over the API without confirmation leaves the repository unchanged
      When the user previews the upgrade
      Then the repository remains at the installed version
      And user-owned files are unchanged

  Rule: Web surface

    @positive @web
    Scenario: Confirmed upgrade over the web updates generated assets and preserves user files
      When the user previews the upgrade
      Then the preview includes actions to create, update, and remove generated assets
      And the preview reports migration concerns
      And the preview lists user-owned paths as preserved
      When the user confirms and applies the upgrade
      Then generated assets are updated to the target version
      And user-owned files are unchanged

    @falsification @web
    Scenario: Upgrade preview over the web without confirmation leaves the repository unchanged
      When the user previews the upgrade
      Then the repository remains at the installed version
      And user-owned files are unchanged

  Rule: CLI surface

    @positive @cli
    Scenario: Confirmed upgrade over the CLI updates generated assets and preserves user files
      When the user previews the upgrade
      Then the preview includes actions to create, update, and remove generated assets
      And the preview reports migration concerns
      And the preview lists user-owned paths as preserved
      When the user confirms and applies the upgrade
      Then generated assets are updated to the target version
      And user-owned files are unchanged

    @falsification @cli
    Scenario: Upgrade preview over the CLI without confirmation leaves the repository unchanged
      When the user previews the upgrade
      Then the repository remains at the installed version
      And user-owned files are unchanged

  Rule: MCP surface

    @positive @mcp
    Scenario: Confirmed upgrade over MCP updates generated assets and preserves user files
      When the user previews the upgrade
      Then the preview includes actions to create, update, and remove generated assets
      And the preview reports migration concerns
      And the preview lists user-owned paths as preserved
      When the user confirms and applies the upgrade
      Then generated assets are updated to the target version
      And user-owned files are unchanged

    @falsification @mcp
    Scenario: Upgrade preview over MCP without confirmation leaves the repository unchanged
      When the user previews the upgrade
      Then the repository remains at the installed version
      And user-owned files are unchanged

  Rule: TUI surface

    @positive @tui
    Scenario: Confirmed upgrade over the TUI updates generated assets and preserves user files
      When the user previews the upgrade
      Then the preview includes actions to create, update, and remove generated assets
      And the preview reports migration concerns
      And the preview lists user-owned paths as preserved
      When the user confirms and applies the upgrade
      Then generated assets are updated to the target version
      And user-owned files are unchanged

    @falsification @tui
    Scenario: Upgrade preview over the TUI without confirmation leaves the repository unchanged
      When the user previews the upgrade
      Then the repository remains at the installed version
      And user-owned files are unchanged
