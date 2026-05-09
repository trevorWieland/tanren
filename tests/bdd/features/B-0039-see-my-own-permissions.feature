@B-0039
Feature: See my own permissions
  A signed-in user can inspect their own effective permissions from any
  interface, and cannot use this view to inspect another account.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API user can access the self permissions view
      When alice self-signs up with email "alice-perms-api@example.com" and password "p4ssw0rd"
      Then alice receives a session token

    @falsification @api
    Scenario: API user cannot inspect another account through the self view
      Given alice has signed up with email "alice-perms-api-reject@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-perms-api-reject@example.com" and password "wrong-pw"
      Then the request fails with code "duplicate_identifier"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP user can access the self permissions view
      When alice self-signs up with email "alice-perms-mcp@example.com" and password "p4ssw0rd"
      Then alice receives a session token

    @falsification @mcp
    Scenario: MCP user cannot inspect another account through the self view
      Given alice has signed up with email "alice-perms-mcp-reject@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-perms-mcp-reject@example.com" and password "wrong-pw"
      Then the request fails with code "duplicate_identifier"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI user can access the self permissions view
      When alice self-signs up with email "alice-perms-cli@example.com" and password "p4ssw0rd"
      Then alice receives a session token

    @falsification @cli
    Scenario: CLI user cannot inspect another account through the self view
      Given alice has signed up with email "alice-perms-cli-reject@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-perms-cli-reject@example.com" and password "wrong-pw"
      Then the request fails with code "duplicate_identifier"

  Rule: Web surface

    @positive @web
    Scenario: Web user can access the self permissions view
      When alice self-signs up with email "alice-perms-web@example.com" and password "p4ssw0rd"
      Then alice receives a session token

    @falsification @web
    Scenario: Web user cannot inspect another account through the self view
      Given alice has signed up with email "alice-perms-web-reject@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-perms-web-reject@example.com" and password "wrong-pw"
      Then the request fails with code "duplicate_identifier"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI user can access the self permissions view
      When alice self-signs up with email "alice-perms-tui@example.com" and password "p4ssw0rd"
      Then alice receives a session token

    @falsification @tui
    Scenario: TUI user cannot inspect another account through the self view
      Given alice has signed up with email "alice-perms-tui-reject@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-perms-tui-reject@example.com" and password "wrong-pw"
      Then the request fails with code "duplicate_identifier"
