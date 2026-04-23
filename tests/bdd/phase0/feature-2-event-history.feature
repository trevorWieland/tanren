@phase0 @wave_a @feature2
Feature: Feature 2 event history is durable, transactional, and replayable

  @positive @BEH-P0-201
  Scenario: 2.1 positive witness - accepted changes emit durable events
    Given a successful mutation
    When the mutation commits
    Then its event(s) are appended durably
    And projections/read-models reflect the same committed truth

  @falsification @BEH-P0-201
  Scenario: 2.1 falsification witness - malformed replay input is rejected safely
    Given malformed or semantically invalid event input
    When replay is attempted
    Then replay fails with explicit diagnostics
    And no partial replay is left behind

  @positive @BEH-P0-202
  Scenario: 2.2 positive witness - replay reconstructs equivalent state
    Given a committed event history
    When state is rebuilt from replay into a clean store
    Then reconstructed state matches the original operational state

  @falsification @BEH-P0-202
  Scenario: 2.2 falsification witness - replay does not accept invalid history
    Given malformed or semantically invalid event input
    When replay is attempted
    Then replay fails with explicit diagnostics
    And no partial replay is left behind

  @positive @BEH-P0-203
  Scenario: 2.3 positive witness - corrupt history fails safely
    Given malformed or semantically invalid event input
    When replay is attempted
    Then replay fails with explicit diagnostics
    And no partial replay is left behind

  @falsification @BEH-P0-203
  Scenario: 2.3 falsification witness - canonical history remains replayable
    Given a committed event history
    When state is rebuilt from replay into a clean store
    Then reconstructed state matches the original operational state
