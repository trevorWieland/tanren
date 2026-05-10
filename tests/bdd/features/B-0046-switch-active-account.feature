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
      And alice sees project availability scoped to the selected account via the api
      When alice switches the active account back to the first account via the api
      Then alice sees the first account as active without re-authentication via the api
      And a "active_account_switched" event is recorded

    @positive @api
    Scenario: API keeps per-window active selection independent
      Given alice holds two signed-in accounts via the api
      When alice switches the active account to the first account in window "11111111-1111-4111-8111-11111111111a" via the api
      And alice switches the active account to the second account in window "11111111-1111-4111-8111-11111111111b" via the api
      Then alice sees different active accounts between windows "11111111-1111-4111-8111-11111111111a" and "11111111-1111-4111-8111-11111111111b" via the api

    @positive @api
    Scenario: API keeps concurrent window switches isolated
      Given alice holds two signed-in accounts via the api
      When alice concurrently switches the active account to the first account in window "11111111-1111-4111-8111-11111111111a" and the second account in window "11111111-1111-4111-8111-11111111111b" via the api
      Then alice sees different active accounts between windows "11111111-1111-4111-8111-11111111111a" and "11111111-1111-4111-8111-11111111111b" via the api

    @falsification @api
    Scenario: API rejects switching to an unsigned account
      Given alice holds one signed-in account via the api
      When alice switches the active account to an unsigned account via the api
      Then the request fails with code "target_account_not_signed_in"
      And a "active_account_switch_rejected" event is recorded

    @falsification @api
    Scenario: API rejects switching with a missing caller session
      Given alice holds two signed-in accounts via the api
      And alice records active-account switch baseline via the api
      And alice invalidates the caller session as "missing" via the api
      When alice switches the active account to the second account via the api
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the api

    @falsification @api
    Scenario: API rejects switching with an expired caller session
      Given alice holds two signed-in accounts via the api
      And alice records active-account switch baseline via the api
      And alice invalidates the caller session as "expired" via the api
      When alice switches the active account to the second account via the api
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the api

    @falsification @api
    Scenario: API rejects switching with a revoked caller session
      Given alice holds two signed-in accounts via the api
      And alice records active-account switch baseline via the api
      And alice invalidates the caller session as "revoked" via the api
      When alice switches the active account to the second account via the api
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the api

    @falsification @api
    Scenario: API rejects non-UUID window ids without mutating active state
      Given alice holds two signed-in accounts via the api
      When alice switches the active account to the second account in window "api-window-invalid" via the api
      Then the request fails with code "validation_failed"
      And alice sees the second account as active via the api

    @falsification @api
    Scenario: API rejects blank window ids without mutating active state
      Given alice holds two signed-in accounts via the api
      When alice switches the active account to the second account in window "" via the api
      Then the request fails with code "validation_failed"
      And alice sees the second account as active via the api

    @falsification @api
    Scenario: API rejects overlong window ids without mutating active state
      Given alice holds two signed-in accounts via the api
      When alice switches the active account to the second account in window "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" via the api
      Then the request fails with code "validation_failed"
      And alice sees the second account as active via the api

    @falsification @api
    Scenario: API switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the api
      When alice switches the active account to the first account in window "11111111-1111-4111-8111-11111111111a" via the api
      And alice switches the active account to the second account in window "11111111-1111-4111-8111-11111111111b" via the api
      Then alice sees window "11111111-1111-4111-8111-11111111111a" stay on the first account after window "11111111-1111-4111-8111-11111111111b" switched via the api

  Rule: Web surface

    @positive @web
    Scenario: Web switches to another signed-in account
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the second account via the web
      Then alice sees the second account as active via the web
      And alice sees project availability scoped to the selected account via the web
      When alice switches the active account back to the first account via the web
      Then alice sees the first account as active without re-authentication via the web
      And a "active_account_switched" event is recorded

    @positive @web
    Scenario: Web keeps per-window active selection independent
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the first account in window "22222222-2222-4222-8222-22222222222a" via the web
      And alice switches the active account to the second account in window "22222222-2222-4222-8222-22222222222b" via the web
      Then alice sees different active accounts between windows "22222222-2222-4222-8222-22222222222a" and "22222222-2222-4222-8222-22222222222b" via the web

    @falsification @web
    Scenario: Web rejects switching to an unsigned account
      Given alice holds one signed-in account via the web
      When alice switches the active account to an unsigned account via the web
      Then the request fails with code "target_account_not_signed_in"
      And a "active_account_switch_rejected" event is recorded

    @falsification @web
    Scenario: Web rejects switching with a missing caller session
      Given alice holds two signed-in accounts via the web
      And alice records active-account switch baseline via the web
      And alice invalidates the caller session as "missing" via the web
      When alice switches the active account to the second account via the web
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the web

    @falsification @web
    Scenario: Web rejects switching with an expired caller session
      Given alice holds two signed-in accounts via the web
      And alice records active-account switch baseline via the web
      And alice invalidates the caller session as "expired" via the web
      When alice switches the active account to the second account via the web
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the web

    @falsification @web
    Scenario: Web rejects switching with a revoked caller session
      Given alice holds two signed-in accounts via the web
      And alice records active-account switch baseline via the web
      And alice invalidates the caller session as "revoked" via the web
      When alice switches the active account to the second account via the web
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the web

    @falsification @web
    Scenario: Web rejects non-UUID window ids without mutating active state
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the second account in window "web-window-invalid" via the web
      Then the request fails with code "validation_failed"
      And alice sees the second account as active via the web

    @falsification @web
    Scenario: Web rejects blank window ids without mutating active state
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the second account in window "" via the web
      Then the request fails with code "validation_failed"
      And alice sees the second account as active via the web

    @falsification @web
    Scenario: Web rejects overlong window ids without mutating active state
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the second account in window "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" via the web
      Then the request fails with code "validation_failed"
      And alice sees the second account as active via the web

    @falsification @web
    Scenario: Web switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the web
      When alice switches the active account to the first account in window "22222222-2222-4222-8222-22222222222a" via the web
      And alice switches the active account to the second account in window "22222222-2222-4222-8222-22222222222b" via the web
      Then alice sees window "22222222-2222-4222-8222-22222222222a" stay on the first account after window "22222222-2222-4222-8222-22222222222b" switched via the web

  Rule: CLI surface

    @positive @cli
    Scenario: CLI switches to another signed-in account
      Given alice holds two signed-in accounts via the cli
      When alice switches the active account to the second account via the cli
      Then alice sees the second account as active via the cli
      And alice sees project availability scoped to the selected account via the cli
      When alice switches the active account back to the first account via the cli
      Then alice sees the first account as active without re-authentication via the cli
      And a "active_account_switched" event is recorded

    @positive @cli
    Scenario: CLI keeps per-window active selection independent
      Given alice holds two signed-in accounts via the cli
      When alice switches the active account to the first account in window "33333333-3333-4333-8333-33333333333a" via the cli
      And alice switches the active account to the second account in window "33333333-3333-4333-8333-33333333333b" via the cli
      Then alice sees different active accounts between windows "33333333-3333-4333-8333-33333333333a" and "33333333-3333-4333-8333-33333333333b" via the cli

    @falsification @cli
    Scenario: CLI rejects switching to an unsigned account
      Given alice holds one signed-in account via the cli
      When alice switches the active account to an unsigned account via the cli
      Then the request fails with code "target_account_not_signed_in"
      And a "active_account_switch_rejected" event is recorded

    @falsification @cli
    Scenario: CLI rejects switching with a missing caller session
      Given alice holds two signed-in accounts via the cli
      And alice records active-account switch baseline via the cli
      And alice invalidates the caller session as "missing" via the cli
      When alice switches the active account to the second account via the cli
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the cli

    @falsification @cli
    Scenario: CLI rejects switching with an expired caller session
      Given alice holds two signed-in accounts via the cli
      And alice records active-account switch baseline via the cli
      And alice invalidates the caller session as "expired" via the cli
      When alice switches the active account to the second account via the cli
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the cli

    @falsification @cli
    Scenario: CLI rejects switching with a revoked caller session
      Given alice holds two signed-in accounts via the cli
      And alice records active-account switch baseline via the cli
      And alice invalidates the caller session as "revoked" via the cli
      When alice switches the active account to the second account via the cli
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the cli

    @falsification @cli
    Scenario: CLI switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the cli
      When alice switches the active account to the first account in window "33333333-3333-4333-8333-33333333333a" via the cli
      And alice switches the active account to the second account in window "33333333-3333-4333-8333-33333333333b" via the cli
      Then alice sees window "33333333-3333-4333-8333-33333333333a" stay on the first account after window "33333333-3333-4333-8333-33333333333b" switched via the cli

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP switches to another signed-in account
      Given alice holds two signed-in accounts via the mcp
      When alice switches the active account to the second account via the mcp
      Then alice sees the second account as active via the mcp
      And alice sees project availability scoped to the selected account via the mcp
      When alice switches the active account back to the first account via the mcp
      Then alice sees the first account as active without re-authentication via the mcp
      And a "active_account_switched" event is recorded

    @positive @mcp
    Scenario: MCP keeps per-window active selection independent
      Given alice holds two signed-in accounts via the mcp
      When alice switches the active account to the first account in window "44444444-4444-4444-8444-44444444444a" via the mcp
      And alice switches the active account to the second account in window "44444444-4444-4444-8444-44444444444b" via the mcp
      Then alice sees different active accounts between windows "44444444-4444-4444-8444-44444444444a" and "44444444-4444-4444-8444-44444444444b" via the mcp

    @falsification @mcp
    Scenario: MCP rejects switching to an unsigned account
      Given alice holds one signed-in account via the mcp
      When alice switches the active account to an unsigned account via the mcp
      Then the request fails with code "target_account_not_signed_in"
      And a "active_account_switch_rejected" event is recorded

    @falsification @mcp
    Scenario: MCP rejects switching with a missing caller session
      Given alice holds two signed-in accounts via the mcp
      And alice records active-account switch baseline via the mcp
      And alice invalidates the caller session as "missing" via the mcp
      When alice switches the active account to the second account via the mcp
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the mcp

    @falsification @mcp
    Scenario: MCP rejects switching with an expired caller session
      Given alice holds two signed-in accounts via the mcp
      And alice records active-account switch baseline via the mcp
      And alice invalidates the caller session as "expired" via the mcp
      When alice switches the active account to the second account via the mcp
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the mcp

    @falsification @mcp
    Scenario: MCP rejects switching with a revoked caller session
      Given alice holds two signed-in accounts via the mcp
      And alice records active-account switch baseline via the mcp
      And alice invalidates the caller session as "revoked" via the mcp
      When alice switches the active account to the second account via the mcp
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the mcp

    @falsification @mcp
    Scenario: MCP switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the mcp
      When alice switches the active account to the first account in window "44444444-4444-4444-8444-44444444444a" via the mcp
      And alice switches the active account to the second account in window "44444444-4444-4444-8444-44444444444b" via the mcp
      Then alice sees window "44444444-4444-4444-8444-44444444444a" stay on the first account after window "44444444-4444-4444-8444-44444444444b" switched via the mcp

  Rule: TUI surface

    @positive @tui
    Scenario: TUI switches to another signed-in account
      Given alice holds two signed-in accounts via the tui
      When alice switches the active account to the second account via the tui
      Then alice sees the second account as active via the tui
      And alice sees project availability scoped to the selected account via the tui
      When alice switches the active account back to the first account via the tui
      Then alice sees the first account as active without re-authentication via the tui
      And a "active_account_switched" event is recorded

    @positive @tui
    Scenario: TUI keeps per-window active selection independent
      Given alice holds two signed-in accounts via the tui
      When alice switches the active account to the first account in window "55555555-5555-4555-8555-55555555555a" via the tui
      And alice switches the active account to the second account in window "55555555-5555-4555-8555-55555555555b" via the tui
      Then alice sees different active accounts between windows "55555555-5555-4555-8555-55555555555a" and "55555555-5555-4555-8555-55555555555b" via the tui

    @falsification @tui
    Scenario: TUI rejects switching to an unsigned account
      Given alice holds one signed-in account via the tui
      When alice switches the active account to an unsigned account via the tui
      Then the request fails with code "target_account_not_signed_in"
      And a "active_account_switch_rejected" event is recorded

    @falsification @tui
    Scenario: TUI rejects switching with a missing caller session
      Given alice holds two signed-in accounts via the tui
      And alice records active-account switch baseline via the tui
      And alice invalidates the caller session as "missing" via the tui
      When alice switches the active account to the second account via the tui
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the tui

    @falsification @tui
    Scenario: TUI rejects switching with an expired caller session
      Given alice holds two signed-in accounts via the tui
      And alice records active-account switch baseline via the tui
      And alice invalidates the caller session as "expired" via the tui
      When alice switches the active account to the second account via the tui
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the tui

    @falsification @tui
    Scenario: TUI rejects switching with a revoked caller session
      Given alice holds two signed-in accounts via the tui
      And alice records active-account switch baseline via the tui
      And alice invalidates the caller session as "revoked" via the tui
      When alice switches the active account to the second account via the tui
      Then the request fails with code "invalid_credential"
      And alice sees no active-account mutation after the rejected switch via the tui

    @falsification @tui
    Scenario: TUI switch in one window does not leak into another window
      Given alice holds two signed-in accounts via the tui
      When alice switches the active account to the first account in window "55555555-5555-4555-8555-55555555555a" via the tui
      And alice switches the active account to the second account in window "55555555-5555-4555-8555-55555555555b" via the tui
      Then alice sees window "55555555-5555-4555-8555-55555555555a" stay on the first account after window "55555555-5555-4555-8555-55555555555b" switched via the tui
