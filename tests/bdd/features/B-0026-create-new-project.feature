@B-0026
Feature: Create a new project from scratch
  A signed-in account can create a new repository at a designated host and
  register it as a selectable Tanren project in one action.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API creates a new project at an accessible designated host
      Given alice has a project account
      And designated fixture host "api-fixture-host" is accessible to alice
      When alice creates new repository "ApiTeam/Greenfield" at designated host "api-fixture-host" as an active project
      Then the project creation succeeds
      And repository "ApiTeam/Greenfield" exists at designated host "api-fixture-host"
      And alice sees repository "apiteam/greenfield" in their project list
      And alice has active project repository "apiteam/greenfield"
      And alice has exactly 1 connected project records
      And repository "apiteam/greenfield" starts with zero Tanren activity counts

    @falsification @api
    Scenario: API rejects project creation at a host without access
      Given alice has a project account
      And designated fixture host "api-private-host" is not accessible to alice
      When alice creates new repository "ApiTeam/Denied" at designated host "api-private-host" as an active project
      Then the project request fails with code "no_access"
      And the project failure summary is "The requested account, host, or repository is not accessible."
      And repository "ApiTeam/Denied" does not exist at designated host "api-private-host"
      And alice has exactly 0 connected project records

    @falsification @api
    Scenario: API rejects duplicate project creation and keeps one registration
      Given alice has a project account
      And designated fixture host "api-fixture-host" is accessible to alice
      When alice creates new repository "ApiTeam/Single" at designated host "api-fixture-host" as an active project
      Then the project creation succeeds
      When alice creates new repository "ApiTeam/Single" at designated host "api-fixture-host" as an active project
      Then the project request fails with code "duplicate_repository"
      And the project failure summary is "A project for the supplied repository already exists in this account."
      And source-control provider create checks were called 1 times
      And repository "ApiTeam/Single" exists at designated host "api-fixture-host"
      And alice has exactly 1 connected project records

    @falsification @api
    Scenario: API redacts internal store failures and compensates created repository side effects
      Given alice has a project account
      And designated fixture host "api-flaky-host" is accessible to alice
      And the project store fails project registration
      When alice creates new repository "ApiTeam/InternalFailure" at designated host "api-flaky-host" as an active project
      Then the project request fails with code "internal_error"
      And the project failure summary is "Tanren encountered an internal error."
      And repository "ApiTeam/InternalFailure" does not exist at designated host "api-flaky-host"

  Rule: Web surface

    @positive @web
    Scenario: Web creates a new project at an accessible designated host
      Given alice has a project account
      And designated fixture host "web-fixture-host" is accessible to alice
      When alice creates new repository "WebTeam/Greenfield" at designated host "web-fixture-host" as an active project
      Then the project creation succeeds
      And repository "WebTeam/Greenfield" exists at designated host "web-fixture-host"
      And alice sees repository "webteam/greenfield" in their project list
      And alice has active project repository "webteam/greenfield"
      And alice has exactly 1 connected project records
      And repository "webteam/greenfield" starts with zero Tanren activity counts

    @falsification @web
    Scenario: Web rejects project creation at a host without access
      Given alice has a project account
      And designated fixture host "web-private-host" is not accessible to alice
      When alice creates new repository "WebTeam/Denied" at designated host "web-private-host" as an active project
      Then the project request fails with code "no_access"
      And the project failure summary is "The requested account, host, or repository is not accessible."
      And repository "WebTeam/Denied" does not exist at designated host "web-private-host"
      And alice has exactly 0 connected project records

    @falsification @web
    Scenario: Web rejects duplicate project creation and keeps one registration
      Given alice has a project account
      And designated fixture host "web-fixture-host" is accessible to alice
      When alice creates new repository "WebTeam/Single" at designated host "web-fixture-host" as an active project
      Then the project creation succeeds
      When alice creates new repository "WebTeam/Single" at designated host "web-fixture-host" as an active project
      Then the project request fails with code "duplicate_repository"
      And the project failure summary is "A project for the supplied repository already exists in this account."
      And source-control provider create checks were called 1 times
      And repository "WebTeam/Single" exists at designated host "web-fixture-host"
      And alice has exactly 1 connected project records

    @falsification @web
    Scenario: Web redacts internal store failures and compensates created repository side effects
      Given alice has a project account
      And designated fixture host "web-flaky-host" is accessible to alice
      And the project store fails project registration
      When alice creates new repository "WebTeam/InternalFailure" at designated host "web-flaky-host" as an active project
      Then the project request fails with code "internal_error"
      And the project failure summary is "Tanren encountered an internal error while processing the request."
      And repository "WebTeam/InternalFailure" does not exist at designated host "web-flaky-host"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI creates a new project at an accessible designated host
      Given alice has a project account
      And designated fixture host "cli-fixture-host" is accessible to alice
      When alice creates new repository "CliTeam/Greenfield" at designated host "cli-fixture-host" as an active project
      Then the project creation succeeds
      And repository "CliTeam/Greenfield" exists at designated host "cli-fixture-host"
      And alice sees repository "cliteam/greenfield" in their project list
      And alice has active project repository "cliteam/greenfield"
      And alice has exactly 1 connected project records
      And repository "cliteam/greenfield" starts with zero Tanren activity counts

    @falsification @cli
    Scenario: CLI rejects project creation at a host without access
      Given alice has a project account
      And designated fixture host "cli-private-host" is not accessible to alice
      When alice creates new repository "CliTeam/Denied" at designated host "cli-private-host" as an active project
      Then the project request fails with code "no_access"
      And the project failure summary is "The requested account, host, or repository is not accessible."
      And repository "CliTeam/Denied" does not exist at designated host "cli-private-host"
      And alice has exactly 0 connected project records

    @falsification @cli
    Scenario: CLI rejects duplicate project creation and keeps one registration
      Given alice has a project account
      And designated fixture host "cli-fixture-host" is accessible to alice
      When alice creates new repository "CliTeam/Single" at designated host "cli-fixture-host" as an active project
      Then the project creation succeeds
      When alice creates new repository "CliTeam/Single" at designated host "cli-fixture-host" as an active project
      Then the project request fails with code "duplicate_repository"
      And the project failure summary is "A project for the supplied repository already exists in this account."
      And repository "CliTeam/Single" exists at designated host "cli-fixture-host"
      And alice has exactly 1 connected project records

    @falsification @cli
    Scenario: CLI redacts internal store failures during project creation
      Given alice has a project account
      And designated fixture host "cli-flaky-host" is accessible to alice
      And the project store fails project registration
      When alice creates new repository "CliTeam/InternalFailure" at designated host "cli-flaky-host" as an active project
      Then the project request fails with code "internal_error"
      And the project failure summary is "Tanren encountered an internal error while processing the project request."
      And repository "CliTeam/InternalFailure" does not exist at designated host "cli-flaky-host"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP creates a new project at an accessible designated host
      Given alice has a project account
      And designated fixture host "mcp-fixture-host" is accessible to alice
      When alice creates new repository "McpTeam/Greenfield" at designated host "mcp-fixture-host" as an active project
      Then the project creation succeeds
      And repository "McpTeam/Greenfield" exists at designated host "mcp-fixture-host"
      And alice sees repository "mcpteam/greenfield" in their project list
      And alice has active project repository "mcpteam/greenfield"
      And alice has exactly 1 connected project records
      And repository "mcpteam/greenfield" starts with zero Tanren activity counts

    @falsification @mcp
    Scenario: MCP rejects project creation at a host without access
      Given alice has a project account
      And designated fixture host "mcp-private-host" is not accessible to alice
      When alice creates new repository "McpTeam/Denied" at designated host "mcp-private-host" as an active project
      Then the project request fails with code "no_access"
      And the project failure summary is "The requested account, host, or repository is not accessible."
      And repository "McpTeam/Denied" does not exist at designated host "mcp-private-host"
      And alice has exactly 0 connected project records

    @falsification @mcp
    Scenario: MCP rejects duplicate project creation and keeps one registration
      Given alice has a project account
      And designated fixture host "mcp-fixture-host" is accessible to alice
      When alice creates new repository "McpTeam/Single" at designated host "mcp-fixture-host" as an active project
      Then the project creation succeeds
      When alice creates new repository "McpTeam/Single" at designated host "mcp-fixture-host" as an active project
      Then the project request fails with code "duplicate_repository"
      And the project failure summary is "A project for the supplied repository already exists in this account."
      And source-control provider create checks were called 1 times
      And repository "McpTeam/Single" exists at designated host "mcp-fixture-host"
      And alice has exactly 1 connected project records

    @falsification @mcp
    Scenario: MCP redacts internal store failures and compensates created repository side effects
      Given alice has a project account
      And designated fixture host "mcp-flaky-host" is accessible to alice
      And the project store fails project registration
      When alice creates new repository "McpTeam/InternalFailure" at designated host "mcp-flaky-host" as an active project
      Then the project request fails with code "internal_error"
      And the project failure summary is "Tanren encountered an internal error while processing the request."
      And repository "McpTeam/InternalFailure" does not exist at designated host "mcp-flaky-host"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI creates a new project at an accessible designated host
      Given alice has a project account
      And designated fixture host "tui-fixture-host" is accessible to alice
      When alice creates new repository "TuiTeam/Greenfield" at designated host "tui-fixture-host" as an active project
      Then the project creation succeeds
      And repository "TuiTeam/Greenfield" exists at designated host "tui-fixture-host"
      And alice sees repository "tuiteam/greenfield" in their project list
      And alice has active project repository "tuiteam/greenfield"
      And alice has exactly 1 connected project records
      And repository "tuiteam/greenfield" starts with zero Tanren activity counts

    @falsification @tui
    Scenario: TUI rejects project creation at a host without access
      Given alice has a project account
      And designated fixture host "tui-private-host" is not accessible to alice
      When alice creates new repository "TuiTeam/Denied" at designated host "tui-private-host" as an active project
      Then the project request fails with code "no_access"
      And the project failure summary is "The requested account, host, or repository is not accessible."
      And repository "TuiTeam/Denied" does not exist at designated host "tui-private-host"
      And alice has exactly 0 connected project records

    @falsification @tui
    Scenario: TUI rejects duplicate project creation and keeps one registration
      Given alice has a project account
      And designated fixture host "tui-fixture-host" is accessible to alice
      When alice creates new repository "TuiTeam/Single" at designated host "tui-fixture-host" as an active project
      Then the project creation succeeds
      When alice creates new repository "TuiTeam/Single" at designated host "tui-fixture-host" as an active project
      Then the project request fails with code "duplicate_repository"
      And the project failure summary is "A project for the supplied repository already exists in this account."
      And source-control provider create checks were called 1 times
      And repository "TuiTeam/Single" exists at designated host "tui-fixture-host"
      And alice has exactly 1 connected project records

    @falsification @tui
    Scenario: TUI redacts internal store failures and compensates created repository side effects
      Given alice has a project account
      And designated fixture host "tui-flaky-host" is accessible to alice
      And the project store fails project registration
      When alice creates new repository "TuiTeam/InternalFailure" at designated host "tui-flaky-host" as an active project
      Then the project request fails with code "internal_error"
      And the project failure summary is "Tanren encountered an internal error while processing the request."
      And repository "TuiTeam/InternalFailure" does not exist at designated host "tui-flaky-host"
