@behavior @installer @cli
Feature: Detect installer drift without mutating files

  @B-0069 @positive
  Scenario: Strict dry-run detects command drift
    Given a bootstrapped rust-cargo repository
    And a rendered command has local drift
    When strict dry-run install is executed
    Then install exits with drift status
    And strict dry-run performs no mutation

  @B-0069 @positive
  Scenario: Strict dry-run detects missing preserved standards
    Given a bootstrapped rust-cargo repository
    And an installed standard is missing
    When strict dry-run install is executed
    Then install exits with drift status
    And strict dry-run performs no mutation

  @B-0069 @falsification
  Scenario: Strict dry-run accepts edited preserved standards
    Given a bootstrapped rust-cargo repository
    And an installed standard has local edits
    When strict dry-run install is executed
    Then install exits successfully
    And strict dry-run performs no mutation

