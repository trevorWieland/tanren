@B-0125
Feature: Store user credentials without exposing secret values
  A user can add, update, and remove their own credentials through every
  Tanren interface. After submission, the stored secret value is never
  returned or projected in any response, view, log, or audit entry.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API user adds a credential and sees only metadata
      When alice self-signs up with email "alice-api-cred@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "my-key"
      Then the response contains kind and scope but no secret value
      When alice lists credentials
      Then every credential shows present status but no secret value

    @falsification @api
    Scenario: API rejects duplicate credential name and records event
      Given alice has signed up with email "alice-api-cred-dup@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "dup-key"
      When alice adds an api_key credential named "dup-key"
      Then the request fails with code "duplicate_credential_name"
      And a "credential_add_rejected" event is recorded

    @falsification @api
    Scenario: API rejects updating another account's credential
      Given alice has signed up with email "alice-api-cred-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-api-cred-cross@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-key"
      When alice attempts to update bob's credential
      Then the request fails with code "unauthorized"
      And a "credential_update_rejected" event is recorded

    @falsification @api
    Scenario: API rejects removing another account's credential
      Given alice has signed up with email "alice-api-cred-rm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-api-cred-rm@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-rm-key"
      When alice attempts to remove bob's credential
      Then the request fails with code "unauthorized"
      And a "credential_remove_rejected" event is recorded

  Rule: Web surface

    @positive @web
    Scenario: Web user adds a credential and sees only metadata
      When alice self-signs up with email "alice-web-cred@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "web-key"
      Then the response contains kind and scope but no secret value

    @falsification @web
    Scenario: Web rejects duplicate credential name
      Given alice has signed up with email "alice-web-cred-dup@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "web-dup"
      When alice adds an api_key credential named "web-dup"
      Then the request fails with code "duplicate_credential_name"

    @falsification @web
    Scenario: Web rejects updating another account's credential
      Given alice has signed up with email "alice-web-cred-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-web-cred-cross@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-web-key"
      When alice attempts to update bob's credential
      Then the request fails with code "unauthorized"

    @falsification @web
    Scenario: Web rejects removing another account's credential
      Given alice has signed up with email "alice-web-cred-rm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-web-cred-rm@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-web-rm"
      When alice attempts to remove bob's credential
      Then the request fails with code "unauthorized"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI user adds a credential and sees only metadata
      When alice self-signs up with email "alice-cli-cred@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "cli-key"
      Then the response contains kind and scope but no secret value

    @falsification @cli
    Scenario: CLI rejects duplicate credential name
      Given alice has signed up with email "alice-cli-cred-dup@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "cli-dup"
      When alice adds an api_key credential named "cli-dup"
      Then the request fails with code "duplicate_credential_name"

    @falsification @cli
    Scenario: CLI rejects updating another account's credential
      Given alice has signed up with email "alice-cli-cred-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-cli-cred-cross@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-cli-key"
      When alice attempts to update bob's credential
      Then the request fails with code "unauthorized"

    @falsification @cli
    Scenario: CLI rejects removing another account's credential
      Given alice has signed up with email "alice-cli-cred-rm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-cli-cred-rm@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-cli-rm"
      When alice attempts to remove bob's credential
      Then the request fails with code "unauthorized"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP user adds a credential and sees only metadata
      When alice self-signs up with email "alice-mcp-cred@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "mcp-key"
      Then the response contains kind and scope but no secret value

    @falsification @mcp
    Scenario: MCP rejects duplicate credential name
      Given alice has signed up with email "alice-mcp-cred-dup@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "mcp-dup"
      When alice adds an api_key credential named "mcp-dup"
      Then the request fails with code "duplicate_credential_name"

    @falsification @mcp
    Scenario: MCP rejects updating another account's credential
      Given alice has signed up with email "alice-mcp-cred-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-mcp-cred-cross@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-mcp-key"
      When alice attempts to update bob's credential
      Then the request fails with code "unauthorized"

    @falsification @mcp
    Scenario: MCP rejects removing another account's credential
      Given alice has signed up with email "alice-mcp-cred-rm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-mcp-cred-rm@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-mcp-rm"
      When alice attempts to remove bob's credential
      Then the request fails with code "unauthorized"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI user adds a credential and sees only metadata
      When alice self-signs up with email "alice-tui-cred@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "tui-key"
      Then the response contains kind and scope but no secret value

    @falsification @tui
    Scenario: TUI rejects duplicate credential name
      Given alice has signed up with email "alice-tui-cred-dup@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "tui-dup"
      When alice adds an api_key credential named "tui-dup"
      Then the request fails with code "duplicate_credential_name"

    @falsification @tui
    Scenario: TUI rejects updating another account's credential
      Given alice has signed up with email "alice-tui-cred-cross@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-tui-cred-cross@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-tui-key"
      When alice attempts to update bob's credential
      Then the request fails with code "unauthorized"

    @falsification @tui
    Scenario: TUI rejects removing another account's credential
      Given alice has signed up with email "alice-tui-cred-rm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-tui-cred-rm@example.com" and password "p4ssw0rd"
      And bob has added an api_key credential named "bob-tui-rm"
      When alice attempts to remove bob's credential
      Then the request fails with code "unauthorized"
