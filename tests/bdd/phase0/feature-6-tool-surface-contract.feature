@phase0 @wave_b @feature6
Feature: Feature 6 tool surface is typed, scoped, and transport-consistent

  @positive @BEH-P0-601
  Scenario: 6.1 positive witness - inputs are validated at the boundary
    Given malformed tool input
    When the tool is invoked at the boundary
    Then it returns a typed validation error
    And no side effect occurs

  @falsification @BEH-P0-601
  Scenario: 6.1 falsification witness - valid input avoids validation failure
    Given valid tool input
    When the tool is invoked at the boundary
    Then no validation error is returned
    And side effects occur only for valid input

  @positive @BEH-P0-602
  Scenario: 6.2 positive witness - capability scoping blocks out-of-phase actions
    Given a phase with a bounded capability set
    When it attempts an out-of-scope tool action
    Then the call is denied with CapabilityDenied
    And no unauthorized mutation is recorded

  @falsification @BEH-P0-602
  Scenario: 6.2 falsification witness - in-scope capability allows the action
    Given a phase with the required capability
    When it performs an in-scope tool action
    Then the call is accepted as in scope
    And exactly one authorized mutation is recorded

  @positive @BEH-P0-603
  Scenario: 6.3 positive witness - MCP and CLI semantics are equivalent
    Given the same valid request semantics
    When executed through MCP and CLI transports
    Then resulting domain effects are equivalent
    And transport-specific wrappers may differ while semantics stay aligned

  @falsification @BEH-P0-603
  Scenario: 6.3 falsification witness - parity mismatch is reported
    Given divergent transport responses for the same semantics
    When parity is evaluated across transports
    Then the mismatch is reported as transport parity drift
    And parity validation does not mark the request equivalent
