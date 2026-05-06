@B-0069
Feature: Detect installer drift without mutating files
  A read-only drift check reports whether installed Tanren assets match
  what a fresh install would produce, without modifying the repository.

  Background:
    Given a freshly installed Tanren repository

  Rule: CLI surface

    @positive @cli
    Scenario: Fresh installed repo reports no drift
      When the drift check runs against the repository
      Then the drift report shows no drift

    @positive @cli
    Scenario: Modified generated asset reports drift
      Given a generated asset is modified
      When the drift check runs against the repository
      Then the drift report shows drift
      And the generated asset is reported as drifted

    @positive @cli
    Scenario: Deleted preserved standard reports missing
      Given a preserved standard is deleted
      When the drift check runs against the repository
      Then the drift report shows the standard as missing
      And the repository is unchanged by the drift check

    @positive @cli
    Scenario: Edited preserved standard is accepted as non-drift
      Given a preserved standard is edited by the user
      When the drift check runs against the repository
      Then the drift report shows the standard as accepted

    @falsification @cli
    Scenario: Drift check leaves the repository unchanged
      Given a generated asset is modified
      When the drift check runs against the repository
      Then the drift report shows drift
      And the repository is unchanged by the drift check

    @falsification @cli
    Scenario: Missing generated asset reported as drift with accepted preserved standard
      Given a generated asset is deleted
      When the drift check runs against the repository
      Then the drift report shows drift
      And the generated asset is reported as missing
      And the preserved standard is reported as accepted
