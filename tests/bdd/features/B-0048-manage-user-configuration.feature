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

    @positive @api
    Scenario: API user adds a credential and sees only metadata
      Given alice has signed up with email "alice-api-cred@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "api-my-key"
      Then the response contains kind and scope but no secret value
      When alice lists credentials
      Then every credential shows present status but no secret value

    @positive @api
    Scenario: API user removes a credential
      Given alice has signed up with email "alice-api-rm@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "api-rm-key"
      When alice removes credential "api-rm-key"
      Then the credential "api-rm-key" is no longer listed

    @positive @api
    Scenario: API teammate on a distinct account does not see alice's user-tier values
      Given alice has signed up with email "alice-api-iso@example.com" and password "p4ssw0rd"
      And alice sets user config "preferred_harness" to "claude"
      And bob has signed up with email "bob-api-iso@example.com" and password "p4ssw0rd"
      When bob lists user config
      Then the list does not include "preferred_harness"

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

    @positive @web
    Scenario: Web user adds a credential and sees only metadata
      Given alice has signed up with email "alice-web-cred@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "web-my-key"
      Then the response contains kind and scope but no secret value

    @positive @web
    Scenario: Web user removes a credential
      Given alice has signed up with email "alice-web-rm@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "web-rm-key"
      When alice removes credential "web-rm-key"
      Then the credential "web-rm-key" is no longer listed

    @positive @web
    Scenario: Web teammate on a distinct account does not see alice's user-tier values
      Given alice has signed up with email "alice-web-iso@example.com" and password "p4ssw0rd"
      And alice sets user config "preferred_provider" to "anthropic"
      And bob has signed up with email "bob-web-iso@example.com" and password "p4ssw0rd"
      When bob lists user config
      Then the list does not include "preferred_provider"

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

    @positive @cli
    Scenario: CLI user adds a credential and sees only metadata
      Given alice has signed up with email "alice-cli-cred@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "cli-my-key"
      Then the response contains kind and scope but no secret value

    @positive @cli
    Scenario: CLI user removes a credential
      Given alice has signed up with email "alice-cli-rm@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "cli-rm-key"
      When alice removes credential "cli-rm-key"
      Then the credential "cli-rm-key" is no longer listed

    @positive @cli
    Scenario: CLI teammate on a distinct account does not see alice's user-tier values
      Given alice has signed up with email "alice-cli-iso@example.com" and password "p4ssw0rd"
      And alice sets user config "preferred_harness" to "codex"
      And bob has signed up with email "bob-cli-iso@example.com" and password "p4ssw0rd"
      When bob lists user config
      Then the list does not include "preferred_harness"

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

    @positive @mcp
    Scenario: MCP user adds a credential and sees only metadata
      Given alice has signed up with email "alice-mcp-cred@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "mcp-my-key"
      Then the response contains kind and scope but no secret value

    @positive @mcp
    Scenario: MCP user removes a credential
      Given alice has signed up with email "alice-mcp-rm@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "mcp-rm-key"
      When alice removes credential "mcp-rm-key"
      Then the credential "mcp-rm-key" is no longer listed

    @positive @mcp
    Scenario: MCP teammate on a distinct account does not see alice's user-tier values
      Given alice has signed up with email "alice-mcp-iso@example.com" and password "p4ssw0rd"
      And alice sets user config "preferred_provider" to "openai"
      And bob has signed up with email "bob-mcp-iso@example.com" and password "p4ssw0rd"
      When bob lists user config
      Then the list does not include "preferred_provider"

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

    @positive @tui
    Scenario: TUI user adds a credential and sees only metadata
      Given alice has signed up with email "alice-tui-cred@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "tui-my-key"
      Then the response contains kind and scope but no secret value

    @positive @tui
    Scenario: TUI user removes a credential
      Given alice has signed up with email "alice-tui-rm@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "tui-rm-key"
      When alice removes credential "tui-rm-key"
      Then the credential "tui-rm-key" is no longer listed

    @positive @tui
    Scenario: TUI teammate on a distinct account does not see alice's user-tier values
      Given alice has signed up with email "alice-tui-iso@example.com" and password "p4ssw0rd"
      And alice sets user config "preferred_harness" to "claude"
      And bob has signed up with email "bob-tui-iso@example.com" and password "p4ssw0rd"
      When bob lists user config
      Then the list does not include "preferred_harness"

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
