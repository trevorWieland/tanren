@B-0026
Feature: Create a new project from scratch
  A solo-builder or team-builder can create a new project from scratch so that a
  brand new repository comes into being at the same time the project is
  registered with Tanren. The new project is immediately selectable as active
  and starts empty — no specs, no milestones, no initiatives. Repository
  creation happens at a designated SCM host the user has access to.

  Background:
    Given a signed-in account "alice"

  Rule: API surface

    @positive @api
    Scenario: Create a new project over the API creates a repo at the designated host and registers the project
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "acme-api-new" at host "git.example.com"
      Then a new repository "acme-api-new" exists at host "git.example.com"
      And the project "acme-api-new" appears in alice's account
      And alice can select "acme-api-new" as her active project
      And the project "acme-api-new" has 0 specs
      And the project "acme-api-new" has 0 milestones
      And the project "acme-api-new" has 0 initiatives

    @falsification @api
    Scenario: Creating a new project at a host the user lacks access to over the API is rejected
      Given a fixture SCM host "git.denied-api.com" that alice cannot access
      When alice creates a new project named "acme-api-denied" at host "git.denied-api.com"
      Then the request fails with code "access_denied"

  Rule: Web surface

    @positive @web
    Scenario: Create a new project over the web creates a repo at the designated host and registers the project
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "acme-web-new" at host "git.example.com"
      Then a new repository "acme-web-new" exists at host "git.example.com"
      And the project "acme-web-new" appears in alice's account
      And alice can select "acme-web-new" as her active project
      And the project "acme-web-new" has 0 specs
      And the project "acme-web-new" has 0 milestones
      And the project "acme-web-new" has 0 initiatives

    @falsification @web
    Scenario: Creating a new project at a host the user lacks access to over the web is rejected
      Given a fixture SCM host "git.denied-web.com" that alice cannot access
      When alice creates a new project named "acme-web-denied" at host "git.denied-web.com"
      Then the request fails with code "access_denied"

  Rule: CLI surface

    @positive @cli
    Scenario: Create a new project over the CLI creates a repo at the designated host and registers the project
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "acme-cli-new" at host "git.example.com"
      Then a new repository "acme-cli-new" exists at host "git.example.com"
      And the project "acme-cli-new" appears in alice's account
      And alice can select "acme-cli-new" as her active project
      And the project "acme-cli-new" has 0 specs
      And the project "acme-cli-new" has 0 milestones
      And the project "acme-cli-new" has 0 initiatives

    @falsification @cli
    Scenario: Creating a new project at a host the user lacks access to over the CLI is rejected
      Given a fixture SCM host "git.denied-cli.com" that alice cannot access
      When alice creates a new project named "acme-cli-denied" at host "git.denied-cli.com"
      Then the request fails with code "access_denied"

  Rule: MCP surface

    @positive @mcp
    Scenario: Create a new project over MCP creates a repo at the designated host and registers the project
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "acme-mcp-new" at host "git.example.com"
      Then a new repository "acme-mcp-new" exists at host "git.example.com"
      And the project "acme-mcp-new" appears in alice's account
      And alice can select "acme-mcp-new" as her active project
      And the project "acme-mcp-new" has 0 specs
      And the project "acme-mcp-new" has 0 milestones
      And the project "acme-mcp-new" has 0 initiatives

    @falsification @mcp
    Scenario: Creating a new project at a host the user lacks access to over MCP is rejected
      Given a fixture SCM host "git.denied-mcp.com" that alice cannot access
      When alice creates a new project named "acme-mcp-denied" at host "git.denied-mcp.com"
      Then the request fails with code "access_denied"

  Rule: TUI surface

    @positive @tui
    Scenario: Create a new project over the TUI creates a repo at the designated host and registers the project
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "acme-tui-new" at host "git.example.com"
      Then a new repository "acme-tui-new" exists at host "git.example.com"
      And the project "acme-tui-new" appears in alice's account
      And alice can select "acme-tui-new" as her active project
      And the project "acme-tui-new" has 0 specs
      And the project "acme-tui-new" has 0 milestones
      And the project "acme-tui-new" has 0 initiatives

    @falsification @tui
    Scenario: Creating a new project at a host the user lacks access to over the TUI is rejected
      Given a fixture SCM host "git.denied-tui.com" that alice cannot access
      When alice creates a new project named "acme-tui-denied" at host "git.denied-tui.com"
      Then the request fails with code "access_denied"

  Rule: Cross-interface verification

    # rationale: a project created through one interface must be observable as the active project through another interface
    @positive @api @cli
    Scenario: A project created via the API is visible and selectable via the CLI with empty state
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "acme-cross-new" at host "git.example.com" over the API
      Then the project "acme-cross-new" is listed when alice lists projects over the CLI
      And alice can select "acme-cross-new" as her active project over the CLI
      And the project "acme-cross-new" has 0 specs over the CLI
      And the project "acme-cross-new" has 0 milestones over the CLI
      And the project "acme-cross-new" has 0 initiatives over the CLI
