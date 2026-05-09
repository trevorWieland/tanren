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
      And alice sees personal and organization availability separated by account via the api
      When alice switches the active account back to the first account via the api
      Then alice sees the first account as active without re-authentication via the api
      And a "active_account_switched" event is recorded

    @positive @api
    Scenario: API keeps per-window active selection independent
      Given alice holds two signed-in accounts via the api
      When alice switches the active account to the first account in window "api-window-a" via the api
      And alice switches the active account to the second account in window "api-window-b" via the api
      Then alice sees different active accounts between windows "api-window-a" and "api-window-b" via the api

    @falsification @api
    Scenario: API rejects switching to an unsigned account
      Given alice holds one signed-in account via the api
      When alice switches the active account to an unsigned account via the api
      Then the request fails with code "target_account_not_signed_in"
      And a "active_account_switch_rejected" event is recorded

    @falsification @api
    Scenario: API switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the api
      When alice switches the active account to the first account in window "api-window-a" via the api
      And alice switches the active account to the second account in window "api-window-b" via the api
      Then alice sees window "api-window-a" stay on the first account after window "api-window-b" switched via the api

  Rule: Web surface

    @positive @web
    Scenario: Web switches to another signed-in account
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the second account via the web
      Then alice sees the second account as active via the web
      And alice sees personal and organization availability separated by account via the web
      When alice switches the active account back to the first account via the web
      Then alice sees the first account as active without re-authentication via the web

    @positive @web
    Scenario: Web keeps per-window active selection independent
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the first account in window "web-window-a" via the web
      And alice switches the active account to the second account in window "web-window-b" via the web
      Then alice sees different active accounts between windows "web-window-a" and "web-window-b" via the web

    @falsification @web
    Scenario: Web rejects switching to an unsigned account
      Given alice holds one signed-in account via the web
      When alice switches the active account to an unsigned account via the web
      Then the request fails with code "target_account_not_signed_in"

    @falsification @web
    Scenario: Web switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the first account in window "web-window-a" via the web
      And alice switches the active account to the second account in window "web-window-b" via the web
      Then alice sees window "web-window-a" stay on the first account after window "web-window-b" switched via the web

  Rule: CLI surface

    @positive @cli
    Scenario: CLI switches to another signed-in account
      Given alice holds two signed-in accounts via the cli
      When alice switches the active account to the second account via the cli
      Then alice sees the second account as active via the cli
      And alice sees personal and organization availability separated by account via the cli
      When alice switches the active account back to the first account via the cli
      Then alice sees the first account as active without re-authentication via the cli

    @positive @cli
    Scenario: CLI keeps per-window active selection independent
      Given alice holds two signed-in accounts via the cli
      When alice switches the active account to the first account in window "cli-window-a" via the cli
      And alice switches the active account to the second account in window "cli-window-b" via the cli
      Then alice sees different active accounts between windows "cli-window-a" and "cli-window-b" via the cli

    @falsification @cli
    Scenario: CLI rejects switching to an unsigned account
      Given alice holds one signed-in account via the cli
      When alice switches the active account to an unsigned account via the cli
      Then the request fails with code "target_account_not_signed_in"

    @falsification @cli
    Scenario: CLI switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the cli
      When alice switches the active account to the first account in window "cli-window-a" via the cli
      And alice switches the active account to the second account in window "cli-window-b" via the cli
      Then alice sees window "cli-window-a" stay on the first account after window "cli-window-b" switched via the cli

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP switches to another signed-in account
      Given alice holds two signed-in accounts via the mcp
      When alice switches the active account to the second account via the mcp
      Then alice sees the second account as active via the mcp
      And alice sees personal and organization availability separated by account via the mcp
      When alice switches the active account back to the first account via the mcp
      Then alice sees the first account as active without re-authentication via the mcp

    @positive @mcp
    Scenario: MCP keeps per-window active selection independent
      Given alice holds two signed-in accounts via the mcp
      When alice switches the active account to the first account in window "mcp-window-a" via the mcp
      And alice switches the active account to the second account in window "mcp-window-b" via the mcp
      Then alice sees different active accounts between windows "mcp-window-a" and "mcp-window-b" via the mcp

    @falsification @mcp
    Scenario: MCP rejects switching to an unsigned account
      Given alice holds one signed-in account via the mcp
      When alice switches the active account to an unsigned account via the mcp
      Then the request fails with code "target_account_not_signed_in"

    @falsification @mcp
    Scenario: MCP switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the mcp
      When alice switches the active account to the first account in window "mcp-window-a" via the mcp
      And alice switches the active account to the second account in window "mcp-window-b" via the mcp
      Then alice sees window "mcp-window-a" stay on the first account after window "mcp-window-b" switched via the mcp

  Rule: TUI surface

    @positive @tui
    Scenario: TUI switches to another signed-in account
      Given alice holds two signed-in accounts via the tui
      When alice switches the active account to the second account via the tui
      Then alice sees the second account as active via the tui
      And alice sees personal and organization availability separated by account via the tui
      When alice switches the active account back to the first account via the tui
      Then alice sees the first account as active without re-authentication via the tui

    @positive @tui
    Scenario: TUI keeps per-window active selection independent
      Given alice holds two signed-in accounts via the tui
      When alice switches the active account to the first account in window "tui-window-a" via the tui
      And alice switches the active account to the second account in window "tui-window-b" via the tui
      Then alice sees different active accounts between windows "tui-window-a" and "tui-window-b" via the tui

    @falsification @tui
    Scenario: TUI rejects switching to an unsigned account
      Given alice holds one signed-in account via the tui
      When alice switches the active account to an unsigned account via the tui
      Then the request fails with code "target_account_not_signed_in"

    @falsification @tui
    Scenario: TUI switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the tui
      When alice switches the active account to the first account in window "tui-window-a" via the tui
      And alice switches the active account to the second account in window "tui-window-b" via the tui
      Then alice sees window "tui-window-a" stay on the first account after window "tui-window-b" switched via the tui
