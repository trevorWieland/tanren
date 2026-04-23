@phase0 @wave_b @feature5
Feature: Feature 5 task completion is monotonic and guard-based

  @positive @BEH-P0-501
  Scenario: 5.1 positive witness - completion requires required guards
    Given a task marked implemented
    When required guards are satisfied in any order
    Then completion occurs only after all required guards converge
    And guard convergence is required for terminal completion

  @falsification @BEH-P0-501
  Scenario: 5.1 falsification witness - incomplete guard set does not complete
    Given a task marked implemented with missing required guards
    When completion is evaluated before guard convergence
    Then completion remains blocked until all required guards converge
    And guard convergence is required for terminal completion

  @positive @BEH-P0-502
  Scenario: 5.2 positive witness - terminal tasks are not reopened
    Given a completed task
    When remediation is needed
    Then remediation is represented as a new task not reopening the completed one
    And the completed task remains terminal and reopen is denied

  @falsification @BEH-P0-502
  Scenario: 5.2 falsification witness - direct reopen attempt is rejected
    Given a completed task with a reopen attempt
    When reopen is attempted directly
    Then the completed task remains terminal and reopen is denied
    And remediation is tracked through a new task with explicit origin
