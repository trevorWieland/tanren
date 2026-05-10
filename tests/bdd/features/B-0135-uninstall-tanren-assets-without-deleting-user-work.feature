@B-0135
Feature: Uninstall Tanren assets without deleting user work
  A builder can preview and apply repository uninstall actions without
  deleting unrelated user-owned work.

  Rule: CLI uninstall preview and preservation witnesses

    @positive @cli
    Scenario: CLI uninstall preview includes Tanren-managed removals
      Given a repository with Tanren-managed assets
      When uninstall preview runs through the cli interface
      Then the preview includes at least one removable Tanren-managed path

    @falsification @cli
    Scenario: CLI uninstall preserves user-owned repository files
      Given a repository with Tanren-managed assets and user-owned files
      When uninstall preview runs through the cli interface
      Then the preview preserves user-owned files

    @falsification @cli
    Scenario: CLI uninstall preview reports nothing to uninstall when manifest is absent
      Given a clean repository fixture
      When tanren-cli uninstall preview runs without confirmation
      Then the uninstall command succeeds
      And the uninstall preview output reports remove, preserve, and warning path lists
      And the uninstall output reports nothing to uninstall
      And no files are written in the repository fixture
      And the install output redacts absolute repository paths

    @falsification @cli
    Scenario: CLI uninstall confirm reports nothing to uninstall when manifest is absent
      Given a clean repository fixture
      When tanren-cli uninstall runs with confirmation
      Then the uninstall command succeeds
      And the uninstall preview output reports remove, preserve, and warning path lists
      And the uninstall apply output reports removed generated and metadata summaries
      And the uninstall output reports nothing to uninstall
      And no files are written in the repository fixture
      And the install output redacts absolute repository paths

    @positive @cli
    Scenario: CLI uninstall confirm removes only managed assets and preserves user work
      Given a repository with Tanren-managed assets and user-owned files
      When tanren-cli uninstall runs with confirmation
      Then the uninstall command succeeds
      And the uninstall preview output reports remove, preserve, and warning path lists
      And the uninstall apply output reports removed generated and metadata summaries
      And the uninstall apply removes generated assets and install metadata
      And repository file "docs/behaviors/user-uninstall-notes.md" preserves its baseline content
      And repository file "crates/tanren-cli-app/src/user_uninstall_notes.rs" preserves its baseline content
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content
      And the install output redacts absolute repository paths
