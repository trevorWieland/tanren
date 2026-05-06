@B-0025
Feature: Connect Tanren to an existing repository
  A solo-builder or team-builder can connect Tanren to a repository that
  already exists so the repository becomes a project. Connection is
  forward-looking — prior commits are not reinterpreted as Tanren activity.
  Each project is scoped to exactly one repository.

  Background:
    Given a signed-in account "alice"

  Rule: API surface

    @positive @api
    Scenario: Connect an accessible repo over the API registers the project and leaves the repo unchanged
      Given an existing repository "acme-api-repo" that alice can access
      When alice connects the repository "acme-api-repo" to her account
      Then the project "acme-api-repo" appears in alice's account
      And alice can select "acme-api-repo" as her active project
      And the repository bytes of "acme-api-repo" are unchanged

    @positive @api
    Scenario: Connecting a repo with prior commits over the API emits no Tanren activity for those commits
      Given an existing repository "acme-api-history-repo" with 5 prior commits that alice can access
      When alice connects the repository "acme-api-history-repo" to her account
      Then no Tanren activity exists for the 5 prior commits

    @positive @api
    Scenario: Connecting two separate repos over the API creates two separate projects
      Given an existing repository "acme-api-poly-a" that alice can access
      And an existing repository "acme-api-poly-b" that alice can access
      When alice connects the repository "acme-api-poly-a" to her account
      And alice connects the repository "acme-api-poly-b" to her account
      Then alice has 2 projects in her account
      And each project is scoped to exactly one repository

    @falsification @api
    Scenario: Connecting a repo without access over the API is rejected
      Given an existing repository "acme-api-denied-repo" that alice cannot access
      When alice connects the repository "acme-api-denied-repo" to her account
      Then the request fails with code "access_denied"

    @falsification @api
    Scenario: Connecting the same repo twice over the API produces a duplicate error
      Given an existing repository "acme-api-dup-repo" that alice can access
      When alice connects the repository "acme-api-dup-repo" to her account
      And alice connects the repository "acme-api-dup-repo" to her account again
      Then the second request fails with code "duplicate_repository"

    @falsification @api
    Scenario: Connecting a repo over the API without an SCM provider configured returns provider_not_configured
      Given no SCM provider is configured
      When alice connects the repository "acme-api-no-provider-repo" to her account
      Then the request fails with code "provider_not_configured"

  Rule: Web surface

    @positive @web
    Scenario: Connect an accessible repo over the web registers the project and leaves the repo unchanged
      Given an existing repository "acme-web-repo" that alice can access
      When alice connects the repository "acme-web-repo" to her account
      Then the project "acme-web-repo" appears in alice's account
      And alice can select "acme-web-repo" as her active project
      And the repository bytes of "acme-web-repo" are unchanged

    @positive @web
    Scenario: Connecting a repo with prior commits over the web emits no Tanren activity for those commits
      Given an existing repository "acme-web-history-repo" with 5 prior commits that alice can access
      When alice connects the repository "acme-web-history-repo" to her account
      Then no Tanren activity exists for the 5 prior commits

    @positive @web
    Scenario: Connecting two separate repos over the web creates two separate projects
      Given an existing repository "acme-web-poly-a" that alice can access
      And an existing repository "acme-web-poly-b" that alice can access
      When alice connects the repository "acme-web-poly-a" to her account
      And alice connects the repository "acme-web-poly-b" to her account
      Then alice has 2 projects in her account
      And each project is scoped to exactly one repository

    @falsification @web
    Scenario: Connecting a repo without access over the web is rejected
      Given an existing repository "acme-web-denied-repo" that alice cannot access
      When alice connects the repository "acme-web-denied-repo" to her account
      Then the request fails with code "access_denied"

    @falsification @web
    Scenario: Connecting the same repo twice over the web produces a duplicate error
      Given an existing repository "acme-web-dup-repo" that alice can access
      When alice connects the repository "acme-web-dup-repo" to her account
      And alice connects the repository "acme-web-dup-repo" to her account again
      Then the second request fails with code "duplicate_repository"

    @falsification @web
    Scenario: Connecting a repo over the web without an SCM provider configured returns provider_not_configured
      Given no SCM provider is configured
      When alice connects the repository "acme-web-no-provider-repo" to her account
      Then the request fails with code "provider_not_configured"

  Rule: CLI surface

    @positive @cli
    Scenario: Connect an accessible repo over the CLI registers the project and leaves the repo unchanged
      Given an existing repository "acme-cli-repo" that alice can access
      When alice connects the repository "acme-cli-repo" to her account
      Then the project "acme-cli-repo" appears in alice's account
      And alice can select "acme-cli-repo" as her active project
      And the repository bytes of "acme-cli-repo" are unchanged

    @positive @cli
    Scenario: Connecting a repo with prior commits over the CLI emits no Tanren activity for those commits
      Given an existing repository "acme-cli-history-repo" with 5 prior commits that alice can access
      When alice connects the repository "acme-cli-history-repo" to her account
      Then no Tanren activity exists for the 5 prior commits

    @positive @cli
    Scenario: Connecting two separate repos over the CLI creates two separate projects
      Given an existing repository "acme-cli-poly-a" that alice can access
      And an existing repository "acme-cli-poly-b" that alice can access
      When alice connects the repository "acme-cli-poly-a" to her account
      And alice connects the repository "acme-cli-poly-b" to her account
      Then alice has 2 projects in her account
      And each project is scoped to exactly one repository

    @falsification @cli
    Scenario: Connecting a repo without access over the CLI is rejected
      Given an existing repository "acme-cli-denied-repo" that alice cannot access
      When alice connects the repository "acme-cli-denied-repo" to her account
      Then the request fails with code "access_denied"

    @falsification @cli
    Scenario: Connecting the same repo twice over the CLI produces a duplicate error
      Given an existing repository "acme-cli-dup-repo" that alice can access
      When alice connects the repository "acme-cli-dup-repo" to her account
      And alice connects the repository "acme-cli-dup-repo" to her account again
      Then the second request fails with code "duplicate_repository"

    @falsification @cli
    Scenario: Connecting a repo over the CLI without an SCM provider configured returns provider_not_configured
      Given no SCM provider is configured
      When alice connects the repository "acme-cli-no-provider-repo" to her account
      Then the request fails with code "provider_not_configured"

  Rule: MCP surface

    @positive @mcp
    Scenario: Connect an accessible repo over MCP registers the project and leaves the repo unchanged
      Given an existing repository "acme-mcp-repo" that alice can access
      When alice connects the repository "acme-mcp-repo" to her account
      Then the project "acme-mcp-repo" appears in alice's account
      And alice can select "acme-mcp-repo" as her active project
      And the repository bytes of "acme-mcp-repo" are unchanged

    @positive @mcp
    Scenario: Connecting a repo with prior commits over MCP emits no Tanren activity for those commits
      Given an existing repository "acme-mcp-history-repo" with 5 prior commits that alice can access
      When alice connects the repository "acme-mcp-history-repo" to her account
      Then no Tanren activity exists for the 5 prior commits

    @positive @mcp
    Scenario: Connecting two separate repos over MCP creates two separate projects
      Given an existing repository "acme-mcp-poly-a" that alice can access
      And an existing repository "acme-mcp-poly-b" that alice can access
      When alice connects the repository "acme-mcp-poly-a" to her account
      And alice connects the repository "acme-mcp-poly-b" to her account
      Then alice has 2 projects in her account
      And each project is scoped to exactly one repository

    @falsification @mcp
    Scenario: Connecting a repo without access over MCP is rejected
      Given an existing repository "acme-mcp-denied-repo" that alice cannot access
      When alice connects the repository "acme-mcp-denied-repo" to her account
      Then the request fails with code "access_denied"

    @falsification @mcp
    Scenario: Connecting the same repo twice over MCP produces a duplicate error
      Given an existing repository "acme-mcp-dup-repo" that alice can access
      When alice connects the repository "acme-mcp-dup-repo" to her account
      And alice connects the repository "acme-mcp-dup-repo" to her account again
      Then the second request fails with code "duplicate_repository"

    @falsification @mcp
    Scenario: Connecting a repo over MCP without an SCM provider configured returns provider_not_configured
      Given no SCM provider is configured
      When alice connects the repository "acme-mcp-no-provider-repo" to her account
      Then the request fails with code "provider_not_configured"

  Rule: TUI surface

    @positive @tui
    Scenario: Connect an accessible repo over the TUI registers the project and leaves the repo unchanged
      Given an existing repository "acme-tui-repo" that alice can access
      When alice connects the repository "acme-tui-repo" to her account
      Then the project "acme-tui-repo" appears in alice's account
      And alice can select "acme-tui-repo" as her active project
      And the repository bytes of "acme-tui-repo" are unchanged

    @positive @tui
    Scenario: Connecting a repo with prior commits over the TUI emits no Tanren activity for those commits
      Given an existing repository "acme-tui-history-repo" with 5 prior commits that alice can access
      When alice connects the repository "acme-tui-history-repo" to her account
      Then no Tanren activity exists for the 5 prior commits

    @positive @tui
    Scenario: Connecting two separate repos over the TUI creates two separate projects
      Given an existing repository "acme-tui-poly-a" that alice can access
      And an existing repository "acme-tui-poly-b" that alice can access
      When alice connects the repository "acme-tui-poly-a" to her account
      And alice connects the repository "acme-tui-poly-b" to her account
      Then alice has 2 projects in her account
      And each project is scoped to exactly one repository

    @falsification @tui
    Scenario: Connecting a repo without access over the TUI is rejected
      Given an existing repository "acme-tui-denied-repo" that alice cannot access
      When alice connects the repository "acme-tui-denied-repo" to her account
      Then the request fails with code "access_denied"

    @falsification @tui
    Scenario: Connecting the same repo twice over the TUI produces a duplicate error
      Given an existing repository "acme-tui-dup-repo" that alice can access
      When alice connects the repository "acme-tui-dup-repo" to her account
      And alice connects the repository "acme-tui-dup-repo" to her account again
      Then the second request fails with code "duplicate_repository"

    @falsification @tui
    Scenario: Connecting a repo over the TUI without an SCM provider configured returns provider_not_configured
      Given no SCM provider is configured
      When alice connects the repository "acme-tui-no-provider-repo" to her account
      Then the request fails with code "provider_not_configured"

  Rule: Input validation

    @falsification @api
    Scenario: Connecting a repository with a URL containing credentials over the API is rejected
      When alice connects a repository with URL "https://user:pass@github.com/org/repo" to her account
      Then the request fails with code "validation_failed"

    @falsification @mcp
    Scenario: Connecting a repository with a URL containing a query string over MCP is rejected
      When alice connects a repository with URL "https://github.com/org/repo?token=secret" to her account
      Then the request fails with code "validation_failed"

    @falsification @cli
    Scenario: Connecting a repository with a URL containing a fragment over the CLI is rejected
      When alice connects a repository with URL "https://github.com/org/repo#section" to her account
      Then the request fails with code "validation_failed"

    @falsification @tui
    Scenario: Connecting a repository with an empty name over the TUI is rejected
      When alice connects a repository with an empty name to her account
      Then the request fails with code "validation_failed"

    @falsification @web
    Scenario: Connecting a repository with a name exceeding the maximum length over the web is rejected
      When alice connects a repository with a name exceeding the maximum length to her account
      Then the request fails with code "validation_failed"

  Rule: Cross-interface verification

    # rationale: project creation via one surface must be observable from another surface without discrepancy
    @positive @api @cli
    Scenario: A project connected via the API is visible and selectable via the CLI
      Given an existing repository "acme-cross-api-cli-repo" that alice can access
      When alice connects the repository "acme-cross-api-cli-repo" to her account over the API
      Then the project "acme-cross-api-cli-repo" is listed when alice lists projects over the CLI
      And alice can select "acme-cross-api-cli-repo" as her active project over the CLI
