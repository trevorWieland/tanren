@phase0 @wave_a @feature3
Feature: Feature 3 contract-derived interface and auth protections

  @positive @BEH-P0-301
  Scenario: 3.1 positive witness - operator can run core dispatch flow end-to-end
    Given a configured environment and valid actor identity
    When an operator performs create, inspect, list, and cancel flows
    Then each action is accepted/rejected according to policy and lifecycle rules
    And observed outcomes match the same domain contract semantics

  @falsification @BEH-P0-301
  Scenario: 3.1 falsification witness - invalid credentials cannot mutate state
    Given invalid identity material or replayed mutation credentials
    When a protected command is attempted
    Then the command is rejected with typed auth/replay errors
    And no unintended mutation occurs

  @positive @BEH-P0-302
  Scenario: 3.2 positive witness - authentication and replay protections are enforced
    Given invalid identity material or replayed mutation credentials
    When a protected command is attempted
    Then the command is rejected with typed auth/replay errors
    And no unintended mutation occurs

  @falsification @BEH-P0-302
  Scenario: 3.2 falsification witness - valid credentials still allow policy-compliant flow
    Given a configured environment and valid actor identity
    When an operator performs create, inspect, list, and cancel flows
    Then each action is accepted/rejected according to policy and lifecycle rules
    And observed outcomes match the same domain contract semantics
