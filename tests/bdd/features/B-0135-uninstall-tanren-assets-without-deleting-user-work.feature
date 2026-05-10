@B-0135
Feature: Uninstall Tanren assets without deleting user work
  A builder can preview and apply repository uninstall actions that remove
  Tanren-generated assets while preserving all user-owned files. Uninstall
  is a repo-local filesystem operation — it does not delete hosted account
  or project history.

  Rule: Positive uninstall witnesses

    @positive @cli
    Scenario: CLI uninstall preview and apply removes generated assets while preserving user-owned files
      Given a repository with Tanren-managed assets and user-owned files
      And repository file "profiles/rust-cargo/global/dependency-management.md" contains "custom standards baseline for uninstall witness"
      And repository file "profiles/rust-cargo/global/dependency-management.md" baseline is recorded
      When uninstall preview runs through the cli interface
      Then the uninstall command succeeds
      And the uninstall preview output reports remove, preserve, and warning path lists
      And no files are written in the repository fixture
      And the uninstall preview preserves user-owned files
      And the install output redacts absolute repository paths
      When uninstall apply runs through the cli interface
      Then the uninstall command succeeds
      And the uninstall apply output reports removed generated and metadata summaries
      And repository file "docs/behaviors/user-uninstall-notes.md" preserves its baseline content
      And repository file "crates/tanren-cli-app/src/user_uninstall_notes.rs" preserves its baseline content
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content
      And the install output redacts absolute repository paths

  Rule: Falsification uninstall witnesses

    @falsification @cli
    Scenario: CLI uninstall preview without confirmation performs no writes and preserves user-owned files
      Given a repository with Tanren-managed assets and user-owned files
      When uninstall preview runs through the cli interface
      Then the uninstall command succeeds
      And the uninstall preview output reports remove, preserve, and warning path lists
      And no files are written in the repository fixture
      And the uninstall preview preserves user-owned files
      And the install output redacts absolute repository paths

    @falsification @cli
    Scenario: CLI repo-local uninstall preview without a prior install reports a clear no-op
      Given a clean repository fixture
      When tanren-cli uninstall preview runs without confirmation
      Then the uninstall command succeeds
      And the uninstall preview output reports remove, preserve, and warning path lists
      And the uninstall output reports nothing to uninstall
      And the uninstall output reports nothing reason manifest_missing
      And no files are written in the repository fixture
      And the install output redacts absolute repository paths

    @falsification @cli
    Scenario: CLI uninstall confirm without a prior install does not exercise hosted account or project history deletion
      Given a clean repository fixture
      When tanren-cli uninstall runs with confirmation
      Then the uninstall command succeeds
      And the uninstall preview output reports remove, preserve, and warning path lists
      And the uninstall apply output reports removed generated and metadata summaries
      And the uninstall output reports nothing to uninstall
      And the uninstall output reports nothing reason manifest_missing
      And no files are written in the repository fixture
      And the install output redacts absolute repository paths
