@B-0135
Feature: Descriptor-driven credential panel workflows
  Signed-in users can complete add/remove credential metadata workflows while
  read paths remain redacted across first-party interfaces.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API credential metadata add and remove flows stay redacted
      Given alice has signed up with email "alice-b0135-api@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "api-b0135-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "api-b0135-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

    @falsification @api
    Scenario: API rejects malformed credential input before persistence
      Given alice has signed up with email "alice-b0135-api-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

  Rule: Web surface

    @positive @web
    Scenario: Web credential metadata add and remove flows stay redacted
      Given alice has signed up with email "alice-b0135-web@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "web-b0135-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "web-b0135-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

    @falsification @web
    Scenario: Web rejects malformed credential input before persistence
      Given alice has signed up with email "alice-b0135-web-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

  Rule: CLI surface

    @positive @cli
    Scenario: CLI credential metadata add and remove flows stay redacted
      Given alice has signed up with email "alice-b0135-cli@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "cli-b0135-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "cli-b0135-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

    @falsification @cli
    Scenario: CLI rejects malformed credential input before persistence
      Given alice has signed up with email "alice-b0135-cli-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP credential metadata add and remove flows stay redacted
      Given alice has signed up with email "alice-b0135-mcp@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "mcp-b0135-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "mcp-b0135-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

    @falsification @mcp
    Scenario: MCP rejects malformed credential input before persistence
      Given alice has signed up with email "alice-b0135-mcp-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

  Rule: TUI surface

    @positive @tui
    Scenario: TUI credential metadata add and remove flows stay redacted
      Given alice has signed up with email "alice-b0135-tui@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "tui-b0135-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "tui-b0135-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows

    @falsification @tui
    Scenario: TUI rejects malformed credential input before persistence
      Given alice has signed up with email "alice-b0135-tui-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows
