@B-0135
Feature: Uninstall Tanren assets without deleting user work
  A builder can preview and apply repository uninstall actions without
  deleting unrelated user-owned work.

  Rule: Interface uninstall preview and preservation witnesses

    @positive @web
    Scenario: Web uninstall preview includes Tanren-managed removals
      Given a repository with Tanren-managed assets
      When uninstall preview runs through the web interface
      Then the preview includes at least one removable Tanren-managed path

    @falsification @web
    Scenario: Web uninstall preserves user-owned repository files
      Given a repository with Tanren-managed assets and user-owned files
      When uninstall preview runs through the web interface
      Then the preview preserves user-owned files

    @positive @api
    Scenario: API uninstall preview includes Tanren-managed removals
      Given a repository with Tanren-managed assets
      When uninstall preview runs through the api interface
      Then the preview includes at least one removable Tanren-managed path

    @falsification @api
    Scenario: API uninstall preserves user-owned repository files
      Given a repository with Tanren-managed assets and user-owned files
      When uninstall preview runs through the api interface
      Then the preview preserves user-owned files

    @positive @mcp
    Scenario: MCP uninstall preview includes Tanren-managed removals
      Given a repository with Tanren-managed assets
      When uninstall preview runs through the mcp interface
      Then the preview includes at least one removable Tanren-managed path

    @falsification @mcp
    Scenario: MCP uninstall preserves user-owned repository files
      Given a repository with Tanren-managed assets and user-owned files
      When uninstall preview runs through the mcp interface
      Then the preview preserves user-owned files

    @positive @cli
    Scenario: CLI uninstall preview includes Tanren-managed removals
      Given a repository with Tanren-managed assets
      When uninstall preview runs through the cli interface
      Then the preview includes at least one removable Tanren-managed path

    @falsification @cli
    Scenario: CLI uninstall preserves user-owned repository files
      Given a repository with Tanren-managed assets and user-owned files
      When uninstall preview runs through the cli interface
      Then the preview preserves user-owned files

    @positive @tui
    Scenario: TUI uninstall preview includes Tanren-managed removals
      Given a repository with Tanren-managed assets
      When uninstall preview runs through the tui interface
      Then the preview includes at least one removable Tanren-managed path

    @falsification @tui
    Scenario: TUI uninstall preserves user-owned repository files
      Given a repository with Tanren-managed assets and user-owned files
      When uninstall preview runs through the tui interface
      Then the preview preserves user-owned files
