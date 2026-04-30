@behavior @finding-lifecycle
Feature: Finding lifecycle and generic check state

  @B-0080 @integration @positive
  Scenario: Task-edit investigation source signals let audit resolve a blocker and reach walk readiness
    Given an initialized finding lifecycle command repository
    When a task-scoped audit finding is investigated
    And do-task records repair source references for the same task
    And audit resolves the finding and completes the task check
    And all spec checks report complete
    Then finding list shows no open fix_now findings
    And investigation attempts list includes source finding history
    And spec status is walk-ready
    And audit artifacts count only open fix_now findings

  @B-0080 @integration @falsification
  Scenario: Historical open fix_now findings block completion and readiness
    Given an initialized finding lifecycle command repository
    When a task-scoped audit finding remains open
    Then audit-task complete is rejected while open blocking findings remain
    And spec status routes task-scoped blockers to task checks

  @B-0080 @integration @falsification
  Scenario: Task-scoped investigate cannot create remediation tasks
    Given an initialized finding lifecycle command repository
    When task-scoped investigate tries to create a task
    Then task creation from task-scoped investigate is rejected

  @B-0080 @integration @positive
  Scenario: Spec-scoped investigate can create follow-up tasks
    Given an initialized finding lifecycle command repository
    When spec-scoped investigate creates a follow-up task
    Then the investigation follow-up task is created

  @B-0080 @integration @positive
  Scenario: Task-scoped investigation routes recovery to the same task
    Given an initialized finding lifecycle command repository
    When a task-scoped audit finding is investigated
    Then spec status routes investigation recovery to the same task

  @B-0080 @integration @positive
  Scenario: Repeated investigation attempts preserve history
    Given an initialized finding lifecycle command repository
    When a task-scoped audit finding is investigated twice
    Then investigation attempts list preserves both attempts

  @B-0080 @integration @falsification
  Scenario: Spec-scoped open findings route to spec checks
    Given an initialized finding lifecycle command repository
    When a spec-scoped audit finding remains open
    Then spec status routes spec-scoped blockers to spec checks

  @B-0080 @integration @positive
  Scenario: Completed tasks can be rechecked after resolving a finding
    Given an initialized finding lifecycle command repository
    When a completed task later has a resolved audit finding
    Then audit-task can report complete for the completed task

  @B-0080 @integration @falsification
  Scenario: Spec checks before the latest task mutation are stale
    Given an initialized finding lifecycle command repository
    When all spec checks reported complete before a later task mutation
    Then spec status requires a fresh spec check batch

  @B-0080 @integration @falsification
  Scenario: do-task cannot resolve findings
    Given an initialized finding lifecycle command repository
    When a task-scoped audit finding remains open
    Then do-task is rejected when it tries to resolve the finding

  @B-0080 @integration @falsification
  Scenario: Investigation links reject dangling provenance
    Given an initialized finding lifecycle command repository
    When an investigation link references missing attempt provenance
    Then investigation provenance linking is rejected

  @B-0080 @unit @positive
  Scenario: Phase0 investigation envelopes preserve typed context
    Given the phase0 orchestrator source
    Then phase0 investigation envelopes include source findings and prior attempts

  @B-0080 @unit @falsification
  Scenario: New lifecycle contracts reject malformed payloads
    Given malformed finding lifecycle and generic check payloads
    Then lifecycle contracts reject unknown fields and invalid links
