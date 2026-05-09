@B-0025
Feature: Connect Tanren to an existing repository
  A signed-in account can connect an existing source-control repository so
  it appears as a selectable Tanren project. Connection is forward-looking:
  prior repository commits are not imported as Tanren activity, and duplicate
  or unauthorized connection attempts are rejected.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API connects an existing repository as an active project
      Given alice has a project account
      And repository fixture "ApiTeam/Atlas" has fingerprint "repo-fp::apiteam/atlas" and 3 prior commits
      When alice connects existing repository "ApiTeam/Atlas" as an active project
      Then the connection succeeds
      And repository "ApiTeam/Atlas" keeps fingerprint "repo-fp::apiteam/atlas"
      And alice sees repository "apiteam/atlas" in their project list
      And alice has active project repository "apiteam/atlas"
      And alice has exactly 1 connected project records
      And repository "apiteam/atlas" has zero Tanren activity counts

    @falsification @api
    Scenario: API rejects connecting a repository without access
      Given repository fixture "ApiTeam/Private" has fingerprint "repo-fp::apiteam/private" and 5 prior commits
      When outsider tries to connect existing repository "ApiTeam/Private" without an account
      Then the project request fails with code "auth_required"

    @falsification @api
    Scenario: API rejects duplicate repository connection and keeps one record
      Given alice has a project account
      And repository fixture "ApiTeam/Single" has fingerprint "repo-fp::apiteam/single" and 2 prior commits
      When alice connects existing repository "ApiTeam/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "ApiTeam/Single" as an active project
      Then the project request fails with code "duplicate_repository"
      And alice has exactly 1 connected project records

    @falsification @api
    Scenario: API does not import prior commits as Tanren activity
      Given alice has a project account
      And repository fixture "ApiTeam/History" has fingerprint "repo-fp::apiteam/history" and 11 prior commits
      When alice connects existing repository "ApiTeam/History" as an active project
      Then the connection succeeds
      And repository "apiteam/history" has zero Tanren activity counts

  Rule: Web surface

    @positive @web
    Scenario: Web connects an existing repository as an active project
      Given alice has a project account
      And repository fixture "WebTeam/Atlas" has fingerprint "repo-fp::webteam/atlas" and 3 prior commits
      When alice connects existing repository "WebTeam/Atlas" as an active project
      Then the connection succeeds
      And repository "WebTeam/Atlas" keeps fingerprint "repo-fp::webteam/atlas"
      And alice sees repository "webteam/atlas" in their project list
      And alice has active project repository "webteam/atlas"
      And alice has exactly 1 connected project records
      And repository "webteam/atlas" has zero Tanren activity counts

    @falsification @web
    Scenario: Web rejects connecting a repository without access
      Given repository fixture "WebTeam/Private" has fingerprint "repo-fp::webteam/private" and 5 prior commits
      When outsider tries to connect existing repository "WebTeam/Private" without an account
      Then the project request fails with code "auth_required"

    @falsification @web
    Scenario: Web rejects duplicate repository connection and keeps one record
      Given alice has a project account
      And repository fixture "WebTeam/Single" has fingerprint "repo-fp::webteam/single" and 2 prior commits
      When alice connects existing repository "WebTeam/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "WebTeam/Single" as an active project
      Then the project request fails with code "duplicate_repository"
      And alice has exactly 1 connected project records

    @falsification @web
    Scenario: Web does not import prior commits as Tanren activity
      Given alice has a project account
      And repository fixture "WebTeam/History" has fingerprint "repo-fp::webteam/history" and 11 prior commits
      When alice connects existing repository "WebTeam/History" as an active project
      Then the connection succeeds
      And repository "webteam/history" has zero Tanren activity counts

  Rule: CLI surface

    @positive @cli
    Scenario: CLI connects an existing repository as an active project
      Given alice has a project account
      And repository fixture "CliTeam/Atlas" has fingerprint "repo-fp::cliteam/atlas" and 3 prior commits
      When alice connects existing repository "CliTeam/Atlas" as an active project
      Then the connection succeeds
      And repository "CliTeam/Atlas" keeps fingerprint "repo-fp::cliteam/atlas"
      And alice sees repository "cliteam/atlas" in their project list
      And alice has active project repository "cliteam/atlas"
      And alice has exactly 1 connected project records
      And repository "cliteam/atlas" has zero Tanren activity counts

    @falsification @cli
    Scenario: CLI rejects connecting a repository without access
      Given repository fixture "CliTeam/Private" has fingerprint "repo-fp::cliteam/private" and 5 prior commits
      When outsider tries to connect existing repository "CliTeam/Private" without an account
      Then the project request fails with code "no_access"

    @falsification @cli
    Scenario: CLI rejects duplicate repository connection and keeps one record
      Given alice has a project account
      And repository fixture "CliTeam/Single" has fingerprint "repo-fp::cliteam/single" and 2 prior commits
      When alice connects existing repository "CliTeam/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "CliTeam/Single" as an active project
      Then the project request fails with code "duplicate_repository"
      And alice has exactly 1 connected project records

    @falsification @cli
    Scenario: CLI does not import prior commits as Tanren activity
      Given alice has a project account
      And repository fixture "CliTeam/History" has fingerprint "repo-fp::cliteam/history" and 11 prior commits
      When alice connects existing repository "CliTeam/History" as an active project
      Then the connection succeeds
      And repository "cliteam/history" has zero Tanren activity counts

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP connects an existing repository as an active project
      Given alice has a project account
      And repository fixture "McpTeam/Atlas" has fingerprint "repo-fp::mcpteam/atlas" and 3 prior commits
      When alice connects existing repository "McpTeam/Atlas" as an active project
      Then the connection succeeds
      And repository "McpTeam/Atlas" keeps fingerprint "repo-fp::mcpteam/atlas"
      And alice sees repository "mcpteam/atlas" in their project list
      And alice has active project repository "mcpteam/atlas"
      And alice has exactly 1 connected project records
      And repository "mcpteam/atlas" has zero Tanren activity counts

    @falsification @mcp
    Scenario: MCP rejects connecting a repository without access
      Given repository fixture "McpTeam/Private" has fingerprint "repo-fp::mcpteam/private" and 5 prior commits
      When outsider tries to connect existing repository "McpTeam/Private" without an account
      Then the project request fails with code "no_access"

    @falsification @mcp
    Scenario: MCP rejects duplicate repository connection and keeps one record
      Given alice has a project account
      And repository fixture "McpTeam/Single" has fingerprint "repo-fp::mcpteam/single" and 2 prior commits
      When alice connects existing repository "McpTeam/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "McpTeam/Single" as an active project
      Then the project request fails with code "duplicate_repository"
      And alice has exactly 1 connected project records

    @falsification @mcp
    Scenario: MCP does not import prior commits as Tanren activity
      Given alice has a project account
      And repository fixture "McpTeam/History" has fingerprint "repo-fp::mcpteam/history" and 11 prior commits
      When alice connects existing repository "McpTeam/History" as an active project
      Then the connection succeeds
      And repository "mcpteam/history" has zero Tanren activity counts

  Rule: TUI surface

    @positive @tui
    Scenario: TUI connects an existing repository as an active project
      Given alice has a project account
      And repository fixture "TuiTeam/Atlas" has fingerprint "repo-fp::tuiteam/atlas" and 3 prior commits
      When alice connects existing repository "TuiTeam/Atlas" as an active project
      Then the connection succeeds
      And repository "TuiTeam/Atlas" keeps fingerprint "repo-fp::tuiteam/atlas"
      And alice sees repository "tuiteam/atlas" in their project list
      And alice has active project repository "tuiteam/atlas"
      And alice has exactly 1 connected project records
      And repository "tuiteam/atlas" has zero Tanren activity counts

    @falsification @tui
    Scenario: TUI rejects connecting a repository without access
      Given repository fixture "TuiTeam/Private" has fingerprint "repo-fp::tuiteam/private" and 5 prior commits
      When outsider tries to connect existing repository "TuiTeam/Private" without an account
      Then the project request fails with code "no_access"

    @falsification @tui
    Scenario: TUI rejects duplicate repository connection and keeps one record
      Given alice has a project account
      And repository fixture "TuiTeam/Single" has fingerprint "repo-fp::tuiteam/single" and 2 prior commits
      When alice connects existing repository "TuiTeam/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "TuiTeam/Single" as an active project
      Then the project request fails with code "duplicate_repository"
      And alice has exactly 1 connected project records

    @falsification @tui
    Scenario: TUI does not import prior commits as Tanren activity
      Given alice has a project account
      And repository fixture "TuiTeam/History" has fingerprint "repo-fp::tuiteam/history" and 11 prior commits
      When alice connects existing repository "TuiTeam/History" as an active project
      Then the connection succeeds
      And repository "tuiteam/history" has zero Tanren activity counts
