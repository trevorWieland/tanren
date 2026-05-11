@B-0065
Feature: See existing members' access to an organization
  A signed-in member of an organization can list every member and see
  each member's organization-level permissions and grant source.
  Non-members are denied access; unsigned calls fail with auth_required.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API lists organization members with permissions and grant source
      Given alice has signed up with email "alice-b0065-api@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha api member org"
      Then the operation succeeds
      When alice lists members of organization "alpha api member org"
      Then the member list includes alice with organization admin permissions
      And the member list includes observation-aligned provenance
      And the member list includes grant source information for each permission

    @falsification @api
    Scenario: API rejects unsigned member list
      Given alice has signed up with email "alice-b0065-api-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "api auth member org"
      Then the operation succeeds
      When alice lists members of organization "api auth member org" without signing in
      Then the request fails with code "auth_required"

    @falsification @api
    Scenario: API rejects non-member member list
      Given alice has signed up with email "alice-b0065-api-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-api-other@example.com" and password "p4ssw0rd"
      When alice creates organization "api owner member org"
      Then the operation succeeds
      When bob lists members of organization "api owner member org"
      Then the request fails with code "permission_denied"

  Rule: Web surface

    @positive @web
    Scenario: Web lists organization members with permissions and grant source
      Given alice has signed up with email "alice-b0065-web@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha web member org"
      Then the operation succeeds
      When alice lists members of organization "alpha web member org"
      Then the member list includes alice with organization admin permissions
      And the member list includes observation-aligned provenance
      And the member list includes grant source information for each permission

    @falsification @web
    Scenario: Web rejects unsigned member list
      Given alice has signed up with email "alice-b0065-web-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "web auth member org"
      Then the operation succeeds
      When alice lists members of organization "web auth member org" without signing in
      Then the request fails with code "auth_required"

    @falsification @web
    Scenario: Web rejects non-member member list
      Given alice has signed up with email "alice-b0065-web-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-web-other@example.com" and password "p4ssw0rd"
      When alice creates organization "web owner member org"
      Then the operation succeeds
      When bob lists members of organization "web owner member org"
      Then the request fails with code "permission_denied"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP lists organization members with permissions and grant source
      Given alice has signed up with email "alice-b0065-mcp@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha mcp member org"
      Then the operation succeeds
      When alice lists members of organization "alpha mcp member org"
      Then the member list includes alice with organization admin permissions
      And the member list includes observation-aligned provenance
      And the member list includes grant source information for each permission

    @falsification @mcp
    Scenario: MCP rejects unsigned member list
      Given alice has signed up with email "alice-b0065-mcp-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "mcp auth member org"
      Then the operation succeeds
      When alice lists members of organization "mcp auth member org" without signing in
      Then the request fails with code "auth_required"

    @falsification @mcp
    Scenario: MCP rejects non-member member list
      Given alice has signed up with email "alice-b0065-mcp-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-mcp-other@example.com" and password "p4ssw0rd"
      When alice creates organization "mcp owner member org"
      Then the operation succeeds
      When bob lists members of organization "mcp owner member org"
      Then the request fails with code "permission_denied"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI lists organization members with permissions and grant source
      Given alice has signed up with email "alice-b0065-cli@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha cli member org"
      Then the operation succeeds
      When alice lists members of organization "alpha cli member org"
      Then the member list includes alice with organization admin permissions
      And the member list includes observation-aligned provenance
      And the member list includes grant source information for each permission

    @falsification @cli
    Scenario: CLI rejects unsigned member list
      Given alice has signed up with email "alice-b0065-cli-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "cli auth member org"
      Then the operation succeeds
      When alice lists members of organization "cli auth member org" without signing in
      Then the request fails with code "auth_required"

    @falsification @cli
    Scenario: CLI rejects non-member member list
      Given alice has signed up with email "alice-b0065-cli-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-cli-other@example.com" and password "p4ssw0rd"
      When alice creates organization "cli owner member org"
      Then the operation succeeds
      When bob lists members of organization "cli owner member org"
      Then the request fails with code "permission_denied"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI lists organization members with permissions and grant source
      Given alice has signed up with email "alice-b0065-tui@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha tui member org"
      Then the operation succeeds
      When alice lists members of organization "alpha tui member org"
      Then the member list includes alice with organization admin permissions
      And the member list includes observation-aligned provenance
      And the member list includes grant source information for each permission

    @falsification @tui
    Scenario: TUI rejects unsigned member list
      Given alice has signed up with email "alice-b0065-tui-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "tui auth member org"
      Then the operation succeeds
      When alice lists members of organization "tui auth member org" without signing in
      Then the request fails with code "auth_required"

    @falsification @tui
    Scenario: TUI rejects non-member member list
      Given alice has signed up with email "alice-b0065-tui-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-tui-other@example.com" and password "p4ssw0rd"
      When alice creates organization "tui owner member org"
      Then the operation succeeds
      When bob lists members of organization "tui owner member org"
      Then the request fails with code "permission_denied"
