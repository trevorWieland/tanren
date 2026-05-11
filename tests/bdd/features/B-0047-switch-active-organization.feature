@B-0047
Feature: Switch the active organization within an account
  A signed-in account that belongs to organizations can switch which
  organization is active. Switching changes which projects are listed
  and which org-level policies apply. Personal accounts with no orgs
  see no organization-scoped actions. Switching to a non-member org
  is rejected with permission_denied.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API switches active org and sees org-scoped context
      Given alice has signed up with email "alice-b0047-api@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha api org"
      Then the operation succeeds
      When alice switches active org to "alpha api org"
      Then the operation succeeds
      And the active org is "alpha api org"
      And the org-scoped capabilities are returned

    @positive @api
    Scenario: API lists switchable organizations for a multi-org account
      Given alice has signed up with email "alice-b0047-api-multi@example.com" and password "p4ssw0rd"
      When alice creates organization "org x api"
      Then the operation succeeds
      When alice creates organization "org y api"
      Then the operation succeeds
      When alice lists active org context
      Then the available organizations include "org x api" and "org y api"

    @positive @api
    Scenario: API personal account sees no org-scoped actions
      Given alice has signed up with email "alice-b0047-api-personal@example.com" and password "p4ssw0rd"
      When alice lists active org context
      Then the active org is none
      And the available organizations list is empty

    @falsification @api
    Scenario: API rejects switching to a non-member organization
      Given alice has signed up with email "alice-b0047-api-nm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0047-api-nm@example.com" and password "p4ssw0rd"
      When alice creates organization "alice private api org"
      Then the operation succeeds
      When bob switches active org to "alice private api org"
      Then the request fails with code "permission_denied"

    @falsification @api
    Scenario: API rejects unsigned org switch and context listing
      Given alice has signed up with email "alice-b0047-api-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned api org"
      Then the operation succeeds
      When alice switches active org to "unsigned api org" without signing in
      Then the request fails with code "auth_required"
      When alice lists active org context without signing in
      Then the request fails with code "auth_required"

  Rule: Web surface

    @positive @web
    Scenario: Web switches active org and sees org-scoped context
      Given alice has signed up with email "alice-b0047-web@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha web org"
      Then the operation succeeds
      When alice switches active org to "alpha web org"
      Then the operation succeeds
      And the active org is "alpha web org"
      And the org-scoped capabilities are returned

    @positive @web
    Scenario: Web lists switchable organizations for a multi-org account
      Given alice has signed up with email "alice-b0047-web-multi@example.com" and password "p4ssw0rd"
      When alice creates organization "org x web"
      Then the operation succeeds
      When alice creates organization "org y web"
      Then the operation succeeds
      When alice lists active org context
      Then the available organizations include "org x web" and "org y web"

    @positive @web
    Scenario: Web personal account sees no org-scoped actions
      Given alice has signed up with email "alice-b0047-web-personal@example.com" and password "p4ssw0rd"
      When alice lists active org context
      Then the active org is none
      And the available organizations list is empty

    @falsification @web
    Scenario: Web rejects switching to a non-member organization
      Given alice has signed up with email "alice-b0047-web-nm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0047-web-nm@example.com" and password "p4ssw0rd"
      When alice creates organization "alice private web org"
      Then the operation succeeds
      When bob switches active org to "alice private web org"
      Then the request fails with code "permission_denied"

    @falsification @web
    Scenario: Web rejects unsigned org switch and context listing
      Given alice has signed up with email "alice-b0047-web-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned web org"
      Then the operation succeeds
      When alice switches active org to "unsigned web org" without signing in
      Then the request fails with code "auth_required"
      When alice lists active org context without signing in
      Then the request fails with code "auth_required"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP switches active org and sees org-scoped context
      Given alice has signed up with email "alice-b0047-mcp@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha mcp org"
      Then the operation succeeds
      When alice switches active org to "alpha mcp org"
      Then the operation succeeds
      And the active org is "alpha mcp org"
      And the org-scoped capabilities are returned

    @positive @mcp
    Scenario: MCP lists switchable organizations for a multi-org account
      Given alice has signed up with email "alice-b0047-mcp-multi@example.com" and password "p4ssw0rd"
      When alice creates organization "org x mcp"
      Then the operation succeeds
      When alice creates organization "org y mcp"
      Then the operation succeeds
      When alice lists active org context
      Then the available organizations include "org x mcp" and "org y mcp"

    @positive @mcp
    Scenario: MCP personal account sees no org-scoped actions
      Given alice has signed up with email "alice-b0047-mcp-personal@example.com" and password "p4ssw0rd"
      When alice lists active org context
      Then the active org is none
      And the available organizations list is empty

    @falsification @mcp
    Scenario: MCP rejects switching to a non-member organization
      Given alice has signed up with email "alice-b0047-mcp-nm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0047-mcp-nm@example.com" and password "p4ssw0rd"
      When alice creates organization "alice private mcp org"
      Then the operation succeeds
      When bob switches active org to "alice private mcp org"
      Then the request fails with code "permission_denied"

    @falsification @mcp
    Scenario: MCP rejects unsigned org switch and context listing
      Given alice has signed up with email "alice-b0047-mcp-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned mcp org"
      Then the operation succeeds
      When alice switches active org to "unsigned mcp org" without signing in
      Then the request fails with code "auth_required"
      When alice lists active org context without signing in
      Then the request fails with code "auth_required"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI switches active org and sees org-scoped context
      Given alice has signed up with email "alice-b0047-cli@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha cli org"
      Then the operation succeeds
      When alice switches active org to "alpha cli org"
      Then the operation succeeds
      And the active org is "alpha cli org"
      And the org-scoped capabilities are returned

    @positive @cli
    Scenario: CLI lists switchable organizations for a multi-org account
      Given alice has signed up with email "alice-b0047-cli-multi@example.com" and password "p4ssw0rd"
      When alice creates organization "org x cli"
      Then the operation succeeds
      When alice creates organization "org y cli"
      Then the operation succeeds
      When alice lists active org context
      Then the available organizations include "org x cli" and "org y cli"

    @positive @cli
    Scenario: CLI personal account sees no org-scoped actions
      Given alice has signed up with email "alice-b0047-cli-personal@example.com" and password "p4ssw0rd"
      When alice lists active org context
      Then the active org is none
      And the available organizations list is empty

    @falsification @cli
    Scenario: CLI rejects switching to a non-member organization
      Given alice has signed up with email "alice-b0047-cli-nm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0047-cli-nm@example.com" and password "p4ssw0rd"
      When alice creates organization "alice private cli org"
      Then the operation succeeds
      When bob switches active org to "alice private cli org"
      Then the request fails with code "permission_denied"

    @falsification @cli
    Scenario: CLI rejects unsigned org switch and context listing
      Given alice has signed up with email "alice-b0047-cli-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned cli org"
      Then the operation succeeds
      When alice switches active org to "unsigned cli org" without signing in
      Then the request fails with code "auth_required"
      When alice lists active org context without signing in
      Then the request fails with code "auth_required"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI switches active org and sees org-scoped context
      Given alice has signed up with email "alice-b0047-tui@example.com" and password "p4ssw0rd"
      When alice creates organization "alpha tui org"
      Then the operation succeeds
      When alice switches active org to "alpha tui org"
      Then the operation succeeds
      And the active org is "alpha tui org"
      And the org-scoped capabilities are returned

    @positive @tui
    Scenario: TUI lists switchable organizations for a multi-org account
      Given alice has signed up with email "alice-b0047-tui-multi@example.com" and password "p4ssw0rd"
      When alice creates organization "org x tui"
      Then the operation succeeds
      When alice creates organization "org y tui"
      Then the operation succeeds
      When alice lists active org context
      Then the available organizations include "org x tui" and "org y tui"

    @positive @tui
    Scenario: TUI personal account sees no org-scoped actions
      Given alice has signed up with email "alice-b0047-tui-personal@example.com" and password "p4ssw0rd"
      When alice lists active org context
      Then the active org is none
      And the available organizations list is empty

    @falsification @tui
    Scenario: TUI rejects switching to a non-member organization
      Given alice has signed up with email "alice-b0047-tui-nm@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-b0047-tui-nm@example.com" and password "p4ssw0rd"
      When alice creates organization "alice private tui org"
      Then the operation succeeds
      When bob switches active org to "alice private tui org"
      Then the request fails with code "permission_denied"

    @falsification @tui
    Scenario: TUI rejects unsigned org switch and context listing
      Given alice has signed up with email "alice-b0047-tui-auth@example.com" and password "p4ssw0rd"
      When alice creates organization "unsigned tui org"
      Then the operation succeeds
      When alice switches active org to "unsigned tui org" without signing in
      Then the request fails with code "auth_required"
      When alice lists active org context without signing in
      Then the request fails with code "auth_required"
