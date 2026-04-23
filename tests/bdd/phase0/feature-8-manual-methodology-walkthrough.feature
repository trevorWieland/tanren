@phase0 @wave_c @feature8
Feature: Feature 8 manual methodology walkthrough is possible now

  @positive @BEH-P0-801
  Scenario: 8.1 positive witness - human-guided end-to-end loop runs in phase 0
    Given a new spec in manual self-hosting mode
    When the 7-step sequence is performed
    Then structured outputs flow through typed tools
    And orchestrator progress/state remains coherent through the loop

  @falsification @BEH-P0-801
  Scenario: 8.1 falsification witness - missing typed transitions break loop coherence
    Given a manual walkthrough with missing typed-tool transitions
    When manual loop coherence is evaluated
    Then inconsistency is detected in task/finding/progress trace
    And walkthrough is not marked coherent
