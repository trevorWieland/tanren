@B-0028
Feature: Switch the active project within an account
  A signed-in user with multiple projects in their active account can
  switch the active project in one step without signing out. After
  switching, spec, loop, and milestone views update to the new project.
  Returning to a prior project resumes its previous view state. Each
  interface in B-0028 (`web`, `api`, `mcp`, `cli`, `tui`) repeats the
  same positive and falsification witnesses required by the spec's
  expected_evidence.

  Background:
    Given a clean Tanren environment
    And account "A" exists with projects "P1", "P2", "P3"
    And project "P1" is the active project with a spec list scrolled to "S5"
    And account "B" exists with project "P4"

  Rule: API surface

    @positive @api
    Scenario: API one-step switch from P1 to P3 updates spec loop and milestone views
      When the user switches the active project from "P1" to "P3" over the API
      Then the active project is "P3"
      And spec list, loop list, and milestone list are scoped to project "P3"

    @positive @api
    Scenario: API returning to P1 resumes prior view state
      When the user switches the active project from "P1" to "P3" over the API
      And the user switches the active project from "P3" to "P1" over the API
      Then the active project is "P1"
      And the spec list is scrolled to "S5" as it was before the switch

    @falsification @api
    Scenario: API rejects switching to a project outside the account
      When the user switches the active project to "P4" over the API
      Then the request fails with code "project_not_found"
      And the active project remains "P1"

    @falsification @api
    Scenario: API switching preserves the previously active project's state
      When the user switches the active project from "P1" to "P3" over the API
      Then the active project is "P3"
      When the user switches the active project from "P3" to "P1" over the API
      Then the spec list is scrolled to "S5" as it was before the switch

  Rule: Web surface

    @positive @web
    Scenario: Web one-step switch from P1 to P3 updates spec loop and milestone views
      When the user switches the active project from "P1" to "P3" on the web
      Then the active project is "P3"
      And spec list, loop list, and milestone list are scoped to project "P3"

    @positive @web
    Scenario: Web switching is usable on a phone-sized viewport
      When the user switches the active project from "P1" to "P3" on the web with a phone-sized viewport
      Then the active project is "P3"
      And the project switcher is accessible without horizontal scrolling

    @positive @web
    Scenario: Web returning to P1 resumes prior view state
      When the user switches the active project from "P1" to "P3" on the web
      And the user switches the active project from "P3" to "P1" on the web
      Then the active project is "P1"
      And the spec list is scrolled to "S5" as it was before the switch

    @falsification @web
    Scenario: Web rejects switching to a project outside the account
      When the user switches the active project to "P4" on the web
      Then the page shows an error indicating the project was not found
      And the active project remains "P1"

    @falsification @web
    Scenario: Web switching preserves the previously active project's state
      When the user switches the active project from "P1" to "P3" on the web
      Then the active project is "P3"
      When the user switches the active project from "P3" to "P1" on the web
      Then the spec list is scrolled to "S5" as it was before the switch

  Rule: CLI surface

    @positive @cli
    Scenario: CLI one-step switch from P1 to P3 updates spec loop and milestone views
      When the user runs the project switch command from "P1" to "P3"
      Then the output confirms the active project is "P3"
      And spec list, loop list, and milestone list are scoped to project "P3"

    @positive @cli
    Scenario: CLI returning to P1 resumes prior view state
      When the user runs the project switch command from "P1" to "P3"
      And the user runs the project switch command from "P3" to "P1"
      Then the output confirms the active project is "P1"
      And the spec list is scrolled to "S5" as it was before the switch

    @falsification @cli
    Scenario: CLI rejects switching to a project outside the account
      When the user runs the project switch command to "P4"
      Then the output contains an error indicating the project was not found
      And the active project remains "P1"

    @falsification @cli
    Scenario: CLI switching preserves the previously active project's state
      When the user runs the project switch command from "P1" to "P3"
      Then the output confirms the active project is "P3"
      When the user runs the project switch command from "P3" to "P1"
      Then the spec list is scrolled to "S5" as it was before the switch

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP one-step switch from P1 to P3 updates spec loop and milestone views
      When the user switches the active project from "P1" to "P3" over MCP
      Then the active project is "P3"
      And spec list, loop list, and milestone list are scoped to project "P3"

    @positive @mcp
    Scenario: MCP returning to P1 resumes prior view state
      When the user switches the active project from "P1" to "P3" over MCP
      And the user switches the active project from "P3" to "P1" over MCP
      Then the active project is "P1"
      And the spec list is scrolled to "S5" as it was before the switch

    @falsification @mcp
    Scenario: MCP rejects switching to a project outside the account
      When the user switches the active project to "P4" over MCP
      Then the request fails with code "project_not_found"
      And the active project remains "P1"

    @falsification @mcp
    Scenario: MCP switching preserves the previously active project's state
      When the user switches the active project from "P1" to "P3" over MCP
      Then the active project is "P3"
      When the user switches the active project from "P3" to "P1" over MCP
      Then the spec list is scrolled to "S5" as it was before the switch

  Rule: TUI surface

    @positive @tui
    Scenario: TUI one-step switch from P1 to P3 updates spec loop and milestone views
      When the user switches the active project from "P1" to "P3" in the TUI
      Then the active project is "P3"
      And spec list, loop list, and milestone list are scoped to project "P3"

    @positive @tui
    Scenario: TUI returning to P1 resumes prior view state
      When the user switches the active project from "P1" to "P3" in the TUI
      And the user switches the active project from "P3" to "P1" in the TUI
      Then the active project is "P1"
      And the spec list is scrolled to "S5" as it was before the switch

    @falsification @tui
    Scenario: TUI rejects switching to a project outside the account
      When the user switches the active project to "P4" in the TUI
      Then the screen shows an error indicating the project was not found
      And the active project remains "P1"

    @falsification @tui
    Scenario: TUI switching preserves the previously active project's state
      When the user switches the active project from "P1" to "P3" in the TUI
      Then the active project is "P3"
      When the user switches the active project from "P3" to "P1" in the TUI
      Then the spec list is scrolled to "S5" as it was before the switch

  Rule: Cross-interface visibility

    # rationale: a project switch performed via API must be observable from the web surface
    @positive @api @web
    Scenario: Switch via API is visible on the web
      When the user switches the active project from "P1" to "P3" over the API
      And the user views the active project on the web
      Then the web shows the active project is "P3"
      And spec list, loop list, and milestone list are scoped to project "P3"
