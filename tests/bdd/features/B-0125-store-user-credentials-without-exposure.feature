@B-0125
Feature: Store user credentials without exposure
  Signed-in users can store user-owned credentials while every interface
  exposes metadata only. Witnesses prove kind/scope/last-updated metadata
  visibility and falsify plaintext leakage through read paths and event
  projections.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API shows redacted credential metadata after create
      Given alice has signed up with email "alice-b0125-api@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "api-b0125-raw-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice sees user credential metadata scoped to their own account
      And alice sees user credential metadata with a last-updated timestamp
      And alice does not see credential value "api-b0125-raw-secret"

    @falsification @api
    Scenario: API blocks and redacts plaintext credential read paths
      Given alice has signed up with email "alice-b0125-api-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0125-api-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with value "api-b0125-sentinel-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user credentials for alice's account
      Then the request fails with code "item_not_found"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows
      When alice signs in with the same credentials
      And alice lists user credentials for their own account
      Then alice sees no credential plaintext value "api-b0125-sentinel-secret" in read paths, logs, audit, or event projections

  Rule: Web surface

    @positive @web
    Scenario: Web shows redacted credential metadata after create
      Given alice has signed up with email "alice-b0125-web@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "web-b0125-raw-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice sees user credential metadata scoped to their own account
      And alice sees user credential metadata with a last-updated timestamp
      And alice does not see credential value "web-b0125-raw-secret"

    @falsification @web
    Scenario: Web blocks and redacts plaintext credential read paths
      Given alice has signed up with email "alice-b0125-web-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0125-web-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with value "web-b0125-sentinel-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user credentials for alice's account
      Then the request fails with code "item_not_found"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows
      When alice signs in with the same credentials
      And alice lists user credentials for their own account
      Then alice sees no credential plaintext value "web-b0125-sentinel-secret" in read paths, logs, audit, or event projections

  Rule: CLI surface

    @positive @cli
    Scenario: CLI shows redacted credential metadata after create
      Given alice has signed up with email "alice-b0125-cli@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "cli-b0125-raw-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice sees user credential metadata scoped to their own account
      And alice sees user credential metadata with a last-updated timestamp
      And alice does not see credential value "cli-b0125-raw-secret"

    @falsification @cli
    Scenario: CLI blocks and redacts plaintext credential read paths
      Given alice has signed up with email "alice-b0125-cli-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0125-cli-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with value "cli-b0125-sentinel-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user credentials for alice's account
      Then the request fails with code "item_not_found"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows
      When alice signs in with the same credentials
      And alice lists user credentials for their own account
      Then alice sees no credential plaintext value "cli-b0125-sentinel-secret" in read paths, logs, audit, or event projections

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP shows redacted credential metadata after create
      Given alice has signed up with email "alice-b0125-mcp@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "mcp-b0125-raw-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice sees user credential metadata scoped to their own account
      And alice sees user credential metadata with a last-updated timestamp
      And alice does not see credential value "mcp-b0125-raw-secret"

    @falsification @mcp
    Scenario: MCP blocks and redacts plaintext credential read paths
      Given alice has signed up with email "alice-b0125-mcp-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0125-mcp-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with value "mcp-b0125-sentinel-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user credentials for alice's account
      Then the request fails with code "item_not_found"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows
      When alice signs in with the same credentials
      And alice lists user credentials for their own account
      Then alice sees no credential plaintext value "mcp-b0125-sentinel-secret" in read paths, logs, audit, or event projections

  Rule: TUI surface

    @positive @tui
    Scenario: TUI shows redacted credential metadata after create
      Given alice has signed up with email "alice-b0125-tui@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a provider_api_token user credential with value "tui-b0125-raw-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice sees user credential metadata scoped to their own account
      And alice sees user credential metadata with a last-updated timestamp
      And alice does not see credential value "tui-b0125-raw-secret"

    @falsification @tui
    Scenario: TUI blocks and redacts plaintext credential read paths
      Given alice has signed up with email "alice-b0125-tui-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0125-tui-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice adds a harness_api_token user credential with value "tui-b0125-sentinel-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user credentials for alice's account
      Then the request fails with code "item_not_found"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows
      When alice signs in with the same credentials
      And alice lists user credentials for their own account
      Then alice sees no credential plaintext value "tui-b0125-sentinel-secret" in read paths, logs, audit, or event projections
