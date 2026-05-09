@B-0046
Feature: Switch the active account
  A signed-in person can list accounts already present in their session
  context and switch the active account per interface. The active-account
  selector rejects targets that are not currently signed in.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API switches to another signed-in account
      Given alice holds two signed-in accounts via the api
      When alice switches the active account to the second account via the api
      Then alice sees the second account as active via the api
      And a "active_account_switched" event is recorded

    @falsification @api
    Scenario: API rejects switching to an unsigned account
      Given alice holds one signed-in account via the api
      When alice switches the active account to an unsigned account via the api
      Then the request fails with code "target_account_not_signed_in"
      And a "active_account_switch_rejected" event is recorded

  Rule: Web surface

    @positive @web
    Scenario: Web switches to another signed-in account
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the second account via the web
      Then alice sees the second account as active via the web

    @falsification @web
    Scenario: Web rejects switching to an unsigned account
      Given alice holds one signed-in account via the web
      When alice switches the active account to an unsigned account via the web
      Then the request fails with code "target_account_not_signed_in"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI switches to another signed-in account
      Given alice holds two signed-in accounts via the cli
      When alice switches the active account to the second account via the cli
      Then alice sees the second account as active via the cli

    @falsification @cli
    Scenario: CLI rejects switching to an unsigned account
      Given alice holds one signed-in account via the cli
      When alice switches the active account to an unsigned account via the cli
      Then the request fails with code "target_account_not_signed_in"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP switches to another signed-in account
      Given alice holds two signed-in accounts via the mcp
      When alice switches the active account to the second account via the mcp
      Then alice sees the second account as active via the mcp

    @falsification @mcp
    Scenario: MCP rejects switching to an unsigned account
      Given alice holds one signed-in account via the mcp
      When alice switches the active account to an unsigned account via the mcp
      Then the request fails with code "target_account_not_signed_in"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI switches to another signed-in account
      Given alice holds two signed-in accounts via the tui
      When alice switches the active account to the second account via the tui
      Then alice sees the second account as active via the tui

    @falsification @tui
    Scenario: TUI rejects switching to an unsigned account
      Given alice holds one signed-in account via the tui
      When alice switches the active account to an unsigned account via the tui
      Then the request fails with code "target_account_not_signed_in"
