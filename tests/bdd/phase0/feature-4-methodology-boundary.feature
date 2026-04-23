@phase0 @wave_b @feature4
Feature: Feature 4 methodology boundary is explicit and enforced

  @positive @BEH-P0-401
  Scenario: 4.1 positive witness - structured workflow state is code-owned
    Given an agent phase operating on a spec
    When structured state must change through typed tools
    Then mutation occurs only through typed tools
    And orchestrator-owned artifacts are not directly agent-edited

  @falsification @BEH-P0-401
  Scenario: 4.1 falsification witness - direct structured-file edit is rejected
    Given an agent attempts direct artifact editing for structured state
    When the edit path is evaluated against methodology boundaries
    Then the direct edit is denied before mutation occurs
    And orchestrator-owned artifacts remain unchanged by the agent

  @positive @BEH-P0-402
  Scenario: 4.2 positive witness - agent markdown remains behavior-only
    Given installed command assets
    When command content is inspected against boundary rules
    Then commands describe behavior and required tool use
    And they do not embed workflow-mechanics ownership responsibilities

  @falsification @BEH-P0-402
  Scenario: 4.2 falsification witness - mechanics ownership content is flagged
    Given command content includes workflow-mechanics ownership instructions
    When command content is inspected against boundary rules
    Then mechanics ownership content is flagged as non-compliant
    And behavior-only command guidance remains required
