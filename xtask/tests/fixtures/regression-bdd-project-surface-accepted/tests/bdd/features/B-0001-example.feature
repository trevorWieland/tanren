@B-0001
Feature: Example gameplay behavior

  @positive @gameplay
  Scenario: Project-defined surface tag is accepted
    Given the example level exists
    When the player loads it
    Then the level is playable
