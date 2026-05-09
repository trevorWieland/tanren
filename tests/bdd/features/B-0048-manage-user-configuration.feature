@B-0048
Feature: Manage user-tier configuration and credentials
  Signed-in users can manage user-tier settings and user-owned
  credential metadata on every first-party interface. Witnesses prove
  metadata-only credential reads, same-account visibility, teammate
  non-visibility, and malformed input rejection before persistence.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API manages user-tier settings and credential metadata
      Given alice has signed up with email "alice-b0048-api@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-api@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees 0 user settings
      When alice sets the theme setting to "dark"
      Then alice sees the theme setting set to "dark"
      When alice adds a provider_api_token user credential with value "api-alice-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "api-alice-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the theme setting set to "dark"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the theme setting set to "dark"

    @falsification @api
    Scenario: API rejects malformed setting input before persistence
      Given alice has signed up with email "alice-b0048-api-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-api-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice sets the editor setting to "nvim"
      And bob signs in with the same credentials
      When bob lists user settings for alice's account
      Then the request fails with code "setting_not_found"
      When bob lists user credentials for alice's account
      Then the request fails with code "item_not_found"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the editor setting set to "nvim"
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the editor setting set to "nvim"
      When bob sets the editor setting to ""
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob sets the editor setting to an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows

  Rule: Web surface
    The browser workflow uses the session-scoped
    `/configuration/account/*` API paths, submits raw credential secrets
    only at credential create/update boundaries, and never sends account
    scope in credential request bodies.

    @positive @web
    Scenario: Web manages user-tier settings and credential metadata
      Given alice has signed up with email "alice-b0048-web@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-web@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees 0 user settings
      When alice sets the theme setting to "dark"
      Then alice sees the theme setting set to "dark"
      When alice adds a provider_api_token user credential with value "web-alice-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "web-alice-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the theme setting set to "dark"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the theme setting set to "dark"

    @falsification @web
    Scenario: Web rejects malformed setting input before persistence
      Given alice has signed up with email "alice-b0048-web-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-web-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice sets the editor setting to "nvim"
      And bob signs in with the same credentials
      When bob lists user settings for alice's account
      Then the request fails with code "setting_not_found"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the editor setting set to "nvim"
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the editor setting set to "nvim"
      When bob sets the editor setting to ""
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob sets the editor setting to an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows

  Rule: CLI surface

    @positive @cli
    Scenario: CLI manages user-tier settings and credential metadata
      Given alice has signed up with email "alice-b0048-cli@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-cli@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees 0 user settings
      When alice sets the theme setting to "dark"
      Then alice sees the theme setting set to "dark"
      When alice adds a provider_api_token user credential with value "cli-alice-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "cli-alice-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the theme setting set to "dark"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the theme setting set to "dark"

    @falsification @cli
    Scenario: CLI rejects malformed setting input before persistence
      Given alice has signed up with email "alice-b0048-cli-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-cli-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice sets the editor setting to "nvim"
      And bob signs in with the same credentials
      When bob lists user settings for alice's account
      Then the request fails with code "setting_not_found"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the editor setting set to "nvim"
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the editor setting set to "nvim"
      When bob sets the editor setting to ""
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob sets the editor setting to an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP manages user-tier settings and credential metadata
      Given alice has signed up with email "alice-b0048-mcp@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-mcp@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees 0 user settings
      When alice sets the theme setting to "dark"
      Then alice sees the theme setting set to "dark"
      When alice adds a provider_api_token user credential with value "mcp-alice-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "mcp-alice-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the theme setting set to "dark"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the theme setting set to "dark"

    @falsification @mcp
    Scenario: MCP rejects malformed setting input before persistence
      Given alice has signed up with email "alice-b0048-mcp-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-mcp-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice sets the editor setting to "nvim"
      And bob signs in with the same credentials
      When bob lists user settings for alice's account
      Then the request fails with code "setting_not_found"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the editor setting set to "nvim"
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the editor setting set to "nvim"
      When bob sets the editor setting to ""
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob sets the editor setting to an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows

  Rule: TUI surface

    @positive @tui
    Scenario: TUI manages user-tier settings and credential metadata
      Given alice has signed up with email "alice-b0048-tui@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-tui@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees 0 user settings
      When alice sets the theme setting to "dark"
      Then alice sees the theme setting set to "dark"
      When alice adds a provider_api_token user credential with value "tui-alice-secret"
      And alice lists user credentials for their own account
      Then alice sees 1 user credential metadata rows
      And alice sees a provider_api_token user credential metadata row
      And alice does not see credential value "tui-alice-secret"
      When alice removes their remembered user credential
      And alice lists user credentials for their own account
      Then alice sees 0 user credential metadata rows
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the theme setting set to "dark"
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the theme setting set to "dark"

    @falsification @tui
    Scenario: TUI rejects malformed setting input before persistence
      Given alice has signed up with email "alice-b0048-tui-fail@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0048-tui-fail@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice sets the editor setting to "nvim"
      And bob signs in with the same credentials
      When bob lists user settings for alice's account
      Then the request fails with code "setting_not_found"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When alice signs in with the same credentials
      And alice lists user settings for their own account
      Then alice sees the editor setting set to "nvim"
      When bob signs in with the same credentials
      And bob lists user settings for their own account
      Then bob does not see the editor setting set to "nvim"
      When bob sets the editor setting to ""
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob sets the editor setting to an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user settings for their own account
      Then bob sees 0 user settings
      When bob adds a harness_api_token user credential with an oversized value
      Then the request fails with code "validation_failed"
      When bob lists user credentials for their own account
      Then bob sees 0 user credential metadata rows
