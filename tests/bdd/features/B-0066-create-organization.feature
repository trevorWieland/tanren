@B-0066
Feature: Create an organization
  A signed-in account can create an organization, list available
  organizations, and verify admin permissions; unsigned calls fail with
  auth_required and non-member permission checks fail with
  permission_denied.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API creates and lists an organization for the signed-in account
      Given alice has signed up with email "alice-b0066-api@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha api org"
      Then the operation succeeds
      And alice holds all organization admin permissions in "alpha api org"
      And organization "alpha api org" has zero initial projects
      When alice lists available organizations
      Then organization "alpha api org" is listed for alice
      When alice checks organization permission "invite" in "alpha api org"
      Then the operation succeeds
      When alice checks organization permission "manage_access" in "alpha api org"
      Then the operation succeeds
      When alice checks organization permission "configure" in "alpha api org"
      Then the operation succeeds
      When alice checks organization permission "set_policy" in "alpha api org"
      Then the operation succeeds

    @falsification @api
    Scenario: API rejects unsigned organization create
      When alice creates organization "unsigned api org" without signing in
      Then the request fails with code "auth_required"

    @falsification @api
    Scenario: API rejects non-member permission check
      Given alice has signed up with email "alice-b0066-api-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0066-api-other@example.com" and password "p4ssw0rd"
      When alice creates organization "api owner org"
      Then the operation succeeds
      When bob checks organization permission "invite" in "api owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "manage_access" in "api owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "configure" in "api owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "set_policy" in "api owner org"
      Then the request fails with code "permission_denied"

  Rule: Web surface

    @positive @web
    Scenario: Web creates and lists an organization for the signed-in account
      Given alice has signed up with email "alice-b0066-web@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha web org"
      Then the operation succeeds
      And alice holds all organization admin permissions in "alpha web org"
      And organization "alpha web org" has zero initial projects
      When alice lists available organizations
      Then organization "alpha web org" is listed for alice
      When alice checks organization permission "invite" in "alpha web org"
      Then the operation succeeds
      When alice checks organization permission "manage_access" in "alpha web org"
      Then the operation succeeds
      When alice checks organization permission "configure" in "alpha web org"
      Then the operation succeeds
      When alice checks organization permission "set_policy" in "alpha web org"
      Then the operation succeeds

    @falsification @web
    Scenario: Web rejects unsigned organization create
      When alice creates organization "unsigned web org" without signing in
      Then the request fails with code "auth_required"

    @falsification @web
    Scenario: Web rejects non-member permission check
      Given alice has signed up with email "alice-b0066-web-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0066-web-other@example.com" and password "p4ssw0rd"
      When alice creates organization "web owner org"
      Then the operation succeeds
      When bob checks organization permission "invite" in "web owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "manage_access" in "web owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "configure" in "web owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "set_policy" in "web owner org"
      Then the request fails with code "permission_denied"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI creates and lists an organization for the signed-in account
      Given alice has signed up with email "alice-b0066-cli@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha-cli-org"
      Then the operation succeeds
      And alice holds all organization admin permissions in "alpha-cli-org"
      And organization "alpha-cli-org" has zero initial projects
      When alice lists available organizations
      Then organization "alpha-cli-org" is listed for alice
      When alice checks organization permission "invite" in "alpha-cli-org"
      Then the operation succeeds
      When alice checks organization permission "manage_access" in "alpha-cli-org"
      Then the operation succeeds
      When alice checks organization permission "configure" in "alpha-cli-org"
      Then the operation succeeds
      When alice checks organization permission "set_policy" in "alpha-cli-org"
      Then the operation succeeds

    @falsification @cli
    Scenario: CLI rejects unsigned organization create
      When alice creates organization "unsigned-cli-org" without signing in
      Then the request fails with code "auth_required"

    @falsification @cli
    Scenario: CLI rejects non-member permission check
      Given alice has signed up with email "alice-b0066-cli-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0066-cli-other@example.com" and password "p4ssw0rd"
      When alice creates organization "cli-owner-org"
      Then the operation succeeds
      When bob checks organization permission "invite" in "cli-owner-org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "manage_access" in "cli-owner-org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "configure" in "cli-owner-org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "set_policy" in "cli-owner-org"
      Then the request fails with code "permission_denied"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP creates and lists an organization for the signed-in account
      Given alice has signed up with email "alice-b0066-mcp@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha mcp org"
      Then the operation succeeds
      And alice holds all organization admin permissions in "alpha mcp org"
      And organization "alpha mcp org" has zero initial projects
      When alice lists available organizations
      Then organization "alpha mcp org" is listed for alice
      When alice checks organization permission "invite" in "alpha mcp org"
      Then the operation succeeds
      When alice checks organization permission "manage_access" in "alpha mcp org"
      Then the operation succeeds
      When alice checks organization permission "configure" in "alpha mcp org"
      Then the operation succeeds
      When alice checks organization permission "set_policy" in "alpha mcp org"
      Then the operation succeeds

    @falsification @mcp
    Scenario: MCP rejects unsigned organization create
      When alice creates organization "unsigned mcp org" without signing in
      Then the request fails with code "auth_required"

    @falsification @mcp
    Scenario: MCP rejects non-member permission check
      Given alice has signed up with email "alice-b0066-mcp-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0066-mcp-other@example.com" and password "p4ssw0rd"
      When alice creates organization "mcp owner org"
      Then the operation succeeds
      When bob checks organization permission "invite" in "mcp owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "manage_access" in "mcp owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "configure" in "mcp owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "set_policy" in "mcp owner org"
      Then the request fails with code "permission_denied"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI creates and lists an organization for the signed-in account
      Given alice has signed up with email "alice-b0066-tui@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha tui org"
      Then the operation succeeds
      And alice holds all organization admin permissions in "alpha tui org"
      And organization "alpha tui org" has zero initial projects
      When alice lists available organizations
      Then organization "alpha tui org" is listed for alice
      When alice checks organization permission "invite" in "alpha tui org"
      Then the operation succeeds
      When alice checks organization permission "manage_access" in "alpha tui org"
      Then the operation succeeds
      When alice checks organization permission "configure" in "alpha tui org"
      Then the operation succeeds
      When alice checks organization permission "set_policy" in "alpha tui org"
      Then the operation succeeds

    @falsification @tui
    Scenario: TUI rejects unsigned organization create
      When alice creates organization "unsigned tui org" without signing in
      Then the request fails with code "auth_required"

    @falsification @tui
    Scenario: TUI rejects non-member permission check
      Given alice has signed up with email "alice-b0066-tui-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0066-tui-other@example.com" and password "p4ssw0rd"
      When alice creates organization "tui owner org"
      Then the operation succeeds
      When bob checks organization permission "invite" in "tui owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "manage_access" in "tui owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "configure" in "tui owner org"
      Then the request fails with code "permission_denied"
      When bob checks organization permission "set_policy" in "tui owner org"
      Then the request fails with code "permission_denied"
