@phase0 @smoke @positive @BEH-P0-401
Feature: Phase 0 BDD harness smoke path

  Scenario: Cucumber harness executes one tagged smoke scenario
    Given the phase0 BDD harness is initialized
    When the smoke scenario executes
    Then the scenario completes successfully
