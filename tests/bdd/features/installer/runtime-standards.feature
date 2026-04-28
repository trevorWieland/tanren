@behavior @installer @cli
Feature: Use the repository's installed standards

  @B-0071 @positive
  Scenario: Runtime standards load from a bootstrapped repository
    Given a bootstrapped rust-cargo repository
    When a methodology command loads runtime standards
    Then install exits successfully

  @B-0071 @falsification
  Scenario: Missing runtime standards root fails explicitly
    Given a bootstrapped rust-cargo repository
    And the runtime standards root is missing
    When a methodology command loads runtime standards
    Then runtime standards loading fails explicitly
