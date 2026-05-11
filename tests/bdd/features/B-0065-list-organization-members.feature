@B-0065
Feature: List organization members
  An authenticated member of an organization can list the members of
  that organization across all surfaces (api, mcp, cli, tui).
  Non-member calls fail with permission_denied; unsigned calls fail
  with auth_required.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API lists members of an organization the caller belongs to
      Given alice has signed up with email "alice-b0065-api@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha api org"
      Then the operation succeeds
      When alice lists members of "alpha api org"
      Then the member list includes alice with admin permissions

    @falsification @api
    Scenario: API rejects unsigned member listing
      Given alice has signed up with email "alice-b0065-api-unsigned@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned api org"
      Then the operation succeeds
      When alice lists members of "unsigned api org" without signing in
      Then the request fails with code "auth_required"

    @falsification @api
    Scenario: API rejects non-member member listing
      Given alice has signed up with email "alice-b0065-api-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-api-other@example.com" and password "p4ssw0rd"
      When alice creates organization "api member org"
      Then the operation succeeds
      When bob lists members of "api member org"
      Then the request fails with code "permission_denied"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP lists members of an organization the caller belongs to
      Given alice has signed up with email "alice-b0065-mcp@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha mcp org"
      Then the operation succeeds
      When alice lists members of "alpha mcp org"
      Then the member list includes alice with admin permissions

    @falsification @mcp
    Scenario: MCP rejects unsigned member listing
      Given alice has signed up with email "alice-b0065-mcp-unsigned@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned mcp org"
      Then the operation succeeds
      When alice lists members of "unsigned mcp org" without signing in
      Then the request fails with code "auth_required"

    @falsification @mcp
    Scenario: MCP rejects non-member member listing
      Given alice has signed up with email "alice-b0065-mcp-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-mcp-other@example.com" and password "p4ssw0rd"
      When alice creates organization "mcp member org"
      Then the operation succeeds
      When bob lists members of "mcp member org"
      Then the request fails with code "permission_denied"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI lists members of an organization the caller belongs to
      Given alice has signed up with email "alice-b0065-cli@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha cli org"
      Then the operation succeeds
      When alice lists members of "alpha cli org"
      Then the member list includes alice with admin permissions

    @falsification @cli
    Scenario: CLI rejects unsigned member listing
      Given alice has signed up with email "alice-b0065-cli-unsigned@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned cli org"
      Then the operation succeeds
      When alice lists members of "unsigned cli org" without signing in
      Then the request fails with code "auth_required"

    @falsification @cli
    Scenario: CLI rejects non-member member listing
      Given alice has signed up with email "alice-b0065-cli-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-cli-other@example.com" and password "p4ssw0rd"
      When alice creates organization "cli member org"
      Then the operation succeeds
      When bob lists members of "cli member org"
      Then the request fails with code "permission_denied"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI lists members of an organization the caller belongs to
      Given alice has signed up with email "alice-b0065-tui@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha tui org"
      Then the operation succeeds
      When alice lists members of "alpha tui org"
      Then the member list includes alice with admin permissions

    @falsification @tui
    Scenario: TUI rejects unsigned member listing
      Given alice has signed up with email "alice-b0065-tui-unsigned@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned tui org"
      Then the operation succeeds
      When alice lists members of "unsigned tui org" without signing in
      Then the request fails with code "auth_required"

    @falsification @tui
    Scenario: TUI rejects non-member member listing
      Given alice has signed up with email "alice-b0065-tui-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-tui-other@example.com" and password "p4ssw0rd"
      When alice creates organization "tui member org"
      Then the operation succeeds
      When bob lists members of "tui member org"
      Then the request fails with code "permission_denied"

  Rule: Web surface

    @positive @web
    Scenario: Web lists members of an organization the caller belongs to
      Given alice has signed up with email "alice-b0065-web@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha web org"
      Then the operation succeeds
      When alice lists members of "alpha web org"
      Then the member list includes alice with admin permissions

    @falsification @web
    Scenario: Web rejects unsigned member listing
      Given alice has signed up with email "alice-b0065-web-unsigned@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned web org"
      Then the operation succeeds
      When alice lists members of "unsigned web org" without signing in
      Then the request fails with code "auth_required"

    @falsification @web
    Scenario: Web rejects non-member member listing
      Given alice has signed up with email "alice-b0065-web-owner@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0065-web-other@example.com" and password "p4ssw0rd"
      When alice creates organization "web member org"
      Then the operation succeeds
      When bob lists members of "web member org"
      Then the request fails with code "permission_denied"
