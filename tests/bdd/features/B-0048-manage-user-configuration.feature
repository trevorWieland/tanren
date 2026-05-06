@B-0048
Feature: Manage user-tier configuration and credentials
  A user can view, set, update, and remove their own user-tier
  configuration values and add, update, and remove user-owned credentials
  through every Tanren interface. Stored credential values are never
  returned in any response or view.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API user views and sets a user-tier configuration value
      When alice self-signs up with email "alice-api-cfg@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice sets user config "preferred_harness" to "claude"
      Then alice's user config "preferred_harness" is "claude"
      When alice lists user config
      Then the list includes "preferred_harness"

    @falsification @api
    Scenario: API rejects malformed user config value and records event
      Given alice has signed up with email "alice-api-cfg-bad@example.com" and password "p4ssw0rd"
      When alice sets user config "preferred_harness" to ""
      Then the request fails with code "invalid_setting_value"
      And a "user_config_set_rejected" event is recorded

    @falsification @api
    Scenario: API rejects viewing another account's user-tier config
      Given alice has signed up with email "alice-api-cfg-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-api-cfg-cross@example.com" and password "p4ssw0rd"
      When alice attempts to read bob's user config "preferred_harness"
      Then the request fails with code "unauthorized"

  Rule: Web surface

    @positive @web
    Scenario: Web user views and sets a user-tier configuration value
      When alice self-signs up with email "alice-web-cfg@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice sets user config "preferred_provider" to "anthropic"
      Then alice's user config "preferred_provider" is "anthropic"

    @falsification @web
    Scenario: Web rejects malformed user config value
      Given alice has signed up with email "alice-web-cfg-bad@example.com" and password "p4ssw0rd"
      When alice sets user config "preferred_provider" to ""
      Then the request fails with code "invalid_setting_value"

    @falsification @web
    Scenario: Web rejects viewing another account's user-tier config
      Given alice has signed up with email "alice-web-cfg-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-web-cfg-cross@example.com" and password "p4ssw0rd"
      When alice attempts to read bob's user config "preferred_provider"
      Then the request fails with code "unauthorized"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI user views and sets a user-tier configuration value
      When alice self-signs up with email "alice-cli-cfg@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice sets user config "preferred_harness" to "codex"
      Then alice's user config "preferred_harness" is "codex"

    @falsification @cli
    Scenario: CLI rejects malformed user config value
      Given alice has signed up with email "alice-cli-cfg-bad@example.com" and password "p4ssw0rd"
      When alice sets user config "preferred_harness" to ""
      Then the request fails with code "invalid_setting_value"

    @falsification @cli
    Scenario: CLI rejects viewing another account's user-tier config
      Given alice has signed up with email "alice-cli-cfg-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-cli-cfg-cross@example.com" and password "p4ssw0rd"
      When alice attempts to read bob's user config "preferred_harness"
      Then the request fails with code "unauthorized"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP user views and sets a user-tier configuration value
      When alice self-signs up with email "alice-mcp-cfg@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice sets user config "preferred_provider" to "openai"
      Then alice's user config "preferred_provider" is "openai"

    @falsification @mcp
    Scenario: MCP rejects malformed user config value
      Given alice has signed up with email "alice-mcp-cfg-bad@example.com" and password "p4ssw0rd"
      When alice sets user config "preferred_provider" to ""
      Then the request fails with code "invalid_setting_value"

    @falsification @mcp
    Scenario: MCP rejects viewing another account's user-tier config
      Given alice has signed up with email "alice-mcp-cfg-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-mcp-cfg-cross@example.com" and password "p4ssw0rd"
      When alice attempts to read bob's user config "preferred_provider"
      Then the request fails with code "unauthorized"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI user views and sets a user-tier configuration value
      When alice self-signs up with email "alice-tui-cfg@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice sets user config "preferred_harness" to "claude"
      Then alice's user config "preferred_harness" is "claude"

    @falsification @tui
    Scenario: TUI rejects malformed user config value
      Given alice has signed up with email "alice-tui-cfg-bad@example.com" and password "p4ssw0rd"
      When alice sets user config "preferred_harness" to ""
      Then the request fails with code "invalid_setting_value"

    @falsification @tui
    Scenario: TUI rejects viewing another account's user-tier config
      Given alice has signed up with email "alice-tui-cfg-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-tui-cfg-cross@example.com" and password "p4ssw0rd"
      When alice attempts to read bob's user config "preferred_harness"
      Then the request fails with code "unauthorized"
