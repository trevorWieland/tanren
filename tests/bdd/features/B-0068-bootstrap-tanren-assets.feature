@B-0068
Feature: Bootstrap Tanren assets into an existing repository
  A builder can run `tanren-cli install` to bootstrap Tanren assets for a
  supported profile, and invalid inputs fail before any repository writes.

  Rule: CLI surface

    @positive @cli
    Scenario: Install Tanren assets into the current repository
      Given a clean repository fixture
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries

    @falsification @cli
    Scenario: Reject install when profile input is invalid
      Given a clean repository fixture
      When tanren-cli install runs with profile "not-a-profile"
      Then the install command exits nonzero
      And the install output reports a validation failure
      And no files are written in the repository fixture
