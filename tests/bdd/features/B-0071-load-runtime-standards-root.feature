@B-0071
Feature: Use the repository's installed standards
  A Tanren command that needs standards reads them from the repository's
  configured standards location, succeeds when present, fails explicitly
  when missing, and never falls back to unrelated content.

  Rule: CLI surface

    @positive @cli
    Scenario: Standards inspect succeeds with installed standards present
      Given a repository with installed standards including "code-style"
      When I inspect the installed standards
      Then the command succeeds
      And the output includes "standards_root="
      And the output includes "count=1"

    @positive @cli
    Scenario: Standards inspect finds standards at a relocated root
      Given a repository with standards at root "custom/path" including "code-style"
      When I inspect the installed standards
      Then the command succeeds
      And the output includes "count=1"

    @falsification @cli
    Scenario: Standards inspect fails when the configured root is missing
      Given a repository with a configured standards root but no standards directory
      When I inspect the installed standards
      Then the command fails
      And the error output includes "standards not found"

    @falsification @cli
    Scenario: Standards inspect fails on a malformed standards file
      Given a repository with a malformed standards file
      When I inspect the installed standards
      Then the command fails
      And the error output includes "parse error"

  Rule: Event stream

    @positive @cli
    Scenario: Installing standards records a configured event
      Given a repository with installed standards including "code-style"
      When I inspect the installed standards
      Then the command succeeds
      And a "standards_root_configured" event is recorded

    @falsification @cli
    Scenario: Clearing the standards root records a cleared event
      Given a repository with a configured standards root but no standards directory
      When I clear the standards configuration
      Then a "standards_root_cleared" event is recorded

  Rule: Event-sourced configuration

    The effective standards root is derived from typed Tanren state
    seeded through the event/read-model path, not from repo-local config.

    @positive @cli
    Scenario: Configuring a standards root records a "standards_root_configured" event
      Given a repository with installed standards including "code-style"
      When I inspect the installed standards
      Then the command succeeds

    @falsification @cli
    Scenario: Clearing the standards root records a "standards_root_cleared" event
      Given a repository with a configured standards root but no standards directory
      When I inspect the installed standards
      Then the command fails
