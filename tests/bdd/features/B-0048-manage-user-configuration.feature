@B-0048
Feature: Manage user-tier configuration and credentials
  Signed-in users can access user-tier configuration surfaces on every
  first-party interface. This wiring-focused proof slice validates that
  each interface reaches authenticated user-scope flows and preserves
  shared failure taxonomy responses.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API user-scope session can be established for configuration management
      Given alice has signed up with email "alice-b0048-api@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      Then alice receives a session token

    @falsification @api
    Scenario: API rejects invalid credentials before user-scope reads
      Given alice has signed up with email "alice-b0048-api-fail@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-b0048-api-fail@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"

  Rule: Web surface

    @positive @web
    Scenario: Web user-scope session can be established for configuration management
      Given alice has signed up with email "alice-b0048-web@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      Then alice receives a session token

    @falsification @web
    Scenario: Web rejects invalid credentials before user-scope reads
      Given alice has signed up with email "alice-b0048-web-fail@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-b0048-web-fail@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI user-scope session can be established for configuration management
      Given alice has signed up with email "alice-b0048-cli@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      Then alice receives a session token

    @falsification @cli
    Scenario: CLI rejects invalid credentials before user-scope reads
      Given alice has signed up with email "alice-b0048-cli-fail@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-b0048-cli-fail@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP user-scope session can be established for configuration management
      Given alice has signed up with email "alice-b0048-mcp@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      Then alice receives a session token

    @falsification @mcp
    Scenario: MCP rejects invalid credentials before user-scope reads
      Given alice has signed up with email "alice-b0048-mcp-fail@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-b0048-mcp-fail@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI user-scope session can be established for configuration management
      Given alice has signed up with email "alice-b0048-tui@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      Then alice receives a session token

    @falsification @tui
    Scenario: TUI rejects invalid credentials before user-scope reads
      Given alice has signed up with email "alice-b0048-tui-fail@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-b0048-tui-fail@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"
