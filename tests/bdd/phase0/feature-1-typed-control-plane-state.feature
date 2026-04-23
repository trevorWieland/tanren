@phase0 @wave_a @feature1
Feature: Feature 1 typed control-plane state is authoritative

  @positive @BEH-P0-101
  Scenario: 1.1 positive witness - valid lifecycle changes are accepted
    Given a new project state
    When a valid sequence of lifecycle mutations is submitted
    Then state advances predictably according to declared transition rules
    And resulting status is consistent across query surfaces

  @falsification @BEH-P0-101
  Scenario: 1.1 falsification witness - invalid lifecycle mutation is rejected
    Given an entity already in a terminal or incompatible state
    When an illegal transition is attempted
    Then the operation is rejected with a typed error
    And no partial state mutation is persisted

  @positive @BEH-P0-102
  Scenario: 1.2 positive witness - invalid lifecycle changes are rejected
    Given an entity already in a terminal or incompatible state
    When an illegal transition is attempted
    Then the operation is rejected with a typed error
    And no partial state mutation is persisted

  @falsification @BEH-P0-102
  Scenario: 1.2 falsification witness - valid lifecycle path remains allowed
    Given a new project state
    When a valid sequence of lifecycle mutations is submitted
    Then state advances predictably according to declared transition rules
    And resulting status is consistent across query surfaces
