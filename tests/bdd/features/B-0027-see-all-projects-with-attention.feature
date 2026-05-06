@B-0027
Feature: See all projects in an account with attention indicators
  A signed-in user can view every project in their active account, see
  which projects need attention (because a spec inside them does), and
  drill into a flagged spec. Each interface in B-0027 (`web`, `api`,
  `mcp`, `cli`, `tui`) repeats the same positive and falsification
  witnesses required by the spec's expected_evidence.

  Background:
    Given a clean Tanren environment
    And account "A" exists with projects "P1", "P2", "P3"
    And project "P2" has a spec "S1" that needs attention
    And account "B" exists with project "P4"

  Rule: API surface

    @positive @api
    Scenario: API list shows every project in the active account with state summaries and aggregated attention on P2
      When the user requests the project list for account "A" over the API
      Then the response contains projects "P1", "P2", "P3"
      And project "P2" shows an attention indicator
      And projects "P1" and "P3" show no attention indicator
      And each project shows a state summary

    @positive @api
    Scenario: API drill-down from project list to P2's flagged spec
      When the user requests the project list for account "A" over the API
      And the user navigates from project "P2" to its flagged spec over the API
      Then the response identifies spec "S1" as needing attention within project "P2"

    @falsification @api
    Scenario: API list excludes projects belonging to another account
      When the user requests the project list for account "A" over the API
      Then the response does not contain project "P4"

    @falsification @api
    Scenario: API project with no attention-worthy specs shows no attention indicator
      Given project "P1" has no specs that need attention
      When the user requests the project list for account "A" over the API
      Then project "P1" shows no attention indicator

  Rule: Web surface

    @positive @web
    Scenario: Web list shows every project in the active account with state summaries and aggregated attention on P2
      When the user views the project list for account "A" on the web
      Then the page shows projects "P1", "P2", "P3"
      And project "P2" shows an attention indicator
      And projects "P1" and "P3" show no attention indicator
      And each project shows a state summary

    @positive @web
    Scenario: Web drill-down from project list to P2's flagged spec
      When the user views the project list for account "A" on the web
      And the user navigates from project "P2" to its flagged spec on the web
      Then the page shows spec "S1" as needing attention within project "P2"

    @positive @web
    Scenario: Web project list is usable on a phone-sized viewport
      When the user views the project list for account "A" on the web with a phone-sized viewport
      Then all project names are fully visible without horizontal scrolling
      And the attention indicator on project "P2" is legible
      And the user can tap project "P2" to navigate to its flagged spec

    @falsification @web
    Scenario: Web list excludes projects belonging to another account
      When the user views the project list for account "A" on the web
      Then the page does not show project "P4"

    @falsification @web
    Scenario: Web project with no attention-worthy specs shows no attention indicator
      Given project "P1" has no specs that need attention
      When the user views the project list for account "A" on the web
      Then project "P1" shows no attention indicator

  Rule: CLI surface

    @positive @cli
    Scenario: CLI list shows every project in the active account with state summaries and aggregated attention on P2
      When the user runs the project list command for account "A"
      Then the output contains projects "P1", "P2", "P3"
      And project "P2" shows an attention indicator
      And projects "P1" and "P3" show no attention indicator
      And each project shows a state summary

    @positive @cli
    Scenario: CLI drill-down from project list to P2's flagged spec
      When the user runs the project list command for account "A"
      And the user navigates from project "P2" to its flagged spec via the CLI
      Then the output identifies spec "S1" as needing attention within project "P2"

    @falsification @cli
    Scenario: CLI list excludes projects belonging to another account
      When the user runs the project list command for account "A"
      Then the output does not contain project "P4"

    @falsification @cli
    Scenario: CLI project with no attention-worthy specs shows no attention indicator
      Given project "P1" has no specs that need attention
      When the user runs the project list command for account "A"
      Then project "P1" shows no attention indicator

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP list shows every project in the active account with state summaries and aggregated attention on P2
      When the user requests the project list for account "A" over MCP
      Then the response contains projects "P1", "P2", "P3"
      And project "P2" shows an attention indicator
      And projects "P1" and "P3" show no attention indicator
      And each project shows a state summary

    @positive @mcp
    Scenario: MCP drill-down from project list to P2's flagged spec
      When the user requests the project list for account "A" over MCP
      And the user navigates from project "P2" to its flagged spec over MCP
      Then the response identifies spec "S1" as needing attention within project "P2"

    @falsification @mcp
    Scenario: MCP list excludes projects belonging to another account
      When the user requests the project list for account "A" over MCP
      Then the response does not contain project "P4"

    @falsification @mcp
    Scenario: MCP project with no attention-worthy specs shows no attention indicator
      Given project "P1" has no specs that need attention
      When the user requests the project list for account "A" over MCP
      Then project "P1" shows no attention indicator

  Rule: TUI surface

    @positive @tui
    Scenario: TUI list shows every project in the active account with state summaries and aggregated attention on P2
      When the user views the project list for account "A" in the TUI
      Then the screen shows projects "P1", "P2", "P3"
      And project "P2" shows an attention indicator
      And projects "P1" and "P3" show no attention indicator
      And each project shows a state summary

    @positive @tui
    Scenario: TUI drill-down from project list to P2's flagged spec
      When the user views the project list for account "A" in the TUI
      And the user navigates from project "P2" to its flagged spec in the TUI
      Then the screen shows spec "S1" as needing attention within project "P2"

    @positive @tui
    Scenario: TUI project list is usable on a small terminal
      When the user views the project list for account "A" in the TUI at 80x24 terminal size
      Then all project names are visible without scrolling horizontally
      And the attention indicator on project "P2" is legible

    @falsification @tui
    Scenario: TUI list excludes projects belonging to another account
      When the user views the project list for account "A" in the TUI
      Then the screen does not show project "P4"

    @falsification @tui
    Scenario: TUI project with no attention-worthy specs shows no attention indicator
      Given project "P1" has no specs that need attention
      When the user views the project list for account "A" in the TUI
      Then project "P1" shows no attention indicator
