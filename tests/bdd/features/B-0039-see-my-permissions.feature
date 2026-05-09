@B-0039
Feature: See my own permissions
  A signed-in user can inspect their own effective permissions from any
  interface, including source and policy-constraint details, and this view
  cannot be used to inspect another account or request grants.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API shows my mixed permission sources and policy constraint
      Given alice has signed up with email "alice-perms-api@example.com" and password "p4ssw0rd"
      And alice has direct and role-template permissions with organization policy reason "Organization policy requires approval ticket."
      When alice views their own permissions
      Then alice sees organization and project permission sections
      And alice sees the direct organization permission entry
      And alice sees the role-template project permission entry
      And alice sees constrained permission reason "Organization policy requires approval ticket." from source "organization_policy"
      And the permissions view does not create permission request or grant events

    @falsification @api
    Scenario: API requires authentication for self permissions
      When an unauthenticated client views their own permissions
      Then the request fails with code "auth_required"
      And the permissions view does not create permission request or grant events

    @falsification @api
    Scenario: API denies requesting another account through the self view
      Given alice has signed up with email "alice-perms-api-deny@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-perms-api-deny@example.com" and password "p4ssw0rd"
      When alice attempts to view bob's permissions through the self view
      Then the request fails with code "permission_denied"
      And the permissions view does not create permission request or grant events

    @positive @api
    Scenario: API discovers that a signed-in actor can open my-permissions navigation
      Given alice has signed up with email "alice-perms-capability-api@example.com" and password "p4ssw0rd"
      When alice discovers their self-permissions capability
      Then the capability indicates my permissions navigation is available

    @falsification @api
    Scenario: API capability discovery denies another account id
      Given alice has signed up with email "alice-perms-capability-api-deny@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-perms-capability-api-deny@example.com" and password "p4ssw0rd"
      When alice attempts to discover bob's permissions capability through the self capability view
      Then the request fails with code "permission_denied"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP shows my mixed permission sources and policy constraint
      Given alice has signed up with email "alice-perms-mcp@example.com" and password "p4ssw0rd"
      And alice has direct and role-template permissions with organization policy reason "Organization policy requires approval ticket."
      When alice views their own permissions
      Then alice sees organization and project permission sections
      And alice sees the direct organization permission entry
      And alice sees the role-template project permission entry
      And alice sees constrained permission reason "Organization policy requires approval ticket." from source "organization_policy"
      And the permissions view does not create permission request or grant events

    @falsification @mcp
    Scenario: MCP requires authentication for self permissions
      When an unauthenticated client views their own permissions
      Then the request fails with code "auth_required"
      And the permissions view does not create permission request or grant events

    @falsification @mcp
    Scenario: MCP denies requesting another account through the self view
      Given alice has signed up with email "alice-perms-mcp-deny@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-perms-mcp-deny@example.com" and password "p4ssw0rd"
      When alice attempts to view bob's permissions through the self view
      Then the request fails with code "permission_denied"
      And the permissions view does not create permission request or grant events

  Rule: CLI surface

    @positive @cli
    Scenario: CLI shows my mixed permission sources and policy constraint
      Given alice has signed up with email "alice-perms-cli@example.com" and password "p4ssw0rd"
      And alice has direct and role-template permissions with organization policy reason "Organization policy requires approval ticket."
      When alice views their own permissions
      Then alice sees organization and project permission sections
      And alice sees the direct organization permission entry
      And alice sees the role-template project permission entry
      And alice sees constrained permission reason "Organization policy requires approval ticket." from source "organization_policy"
      And the permissions view does not create permission request or grant events

    @falsification @cli
    Scenario: CLI requires authentication for self permissions
      When an unauthenticated client views their own permissions
      Then the request fails with code "auth_required"
      And the permissions view does not create permission request or grant events

    @falsification @cli
    Scenario: CLI denies requesting another account through the self view
      Given alice has signed up with email "alice-perms-cli-deny@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-perms-cli-deny@example.com" and password "p4ssw0rd"
      When alice attempts to view bob's permissions through the self view
      Then the request fails with code "permission_denied"
      And the permissions view does not create permission request or grant events

  Rule: Web surface

    @positive @web
    Scenario: Web shows my mixed permission sources and policy constraint, including phone
      Given alice has signed up with email "alice-perms-web@example.com" and password "p4ssw0rd"
      And alice has direct and role-template permissions with organization policy reason "Organization policy requires approval ticket."
      When alice views their own permissions
      Then alice sees organization and project permission sections
      And alice sees the direct organization permission entry
      And alice sees the role-template project permission entry
      And alice sees constrained permission reason "Organization policy requires approval ticket." from source "organization_policy"
      And on a phone viewport the web permissions page shows the role-template source, source proof references, and constraint reason
      And the permissions view does not create permission request or grant events

    @falsification @web
    Scenario: Web requires authentication for self permissions
      When an unauthenticated client views their own permissions
      Then the request fails with code "auth_required"
      And the permissions view does not create permission request or grant events

    @falsification @web
    Scenario: Web denies requesting another account through the self view
      Given alice has signed up with email "alice-perms-web-deny@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-perms-web-deny@example.com" and password "p4ssw0rd"
      When alice attempts to view bob's permissions through the self view
      Then the request fails with code "permission_denied"
      And the permissions view does not create permission request or grant events

    @positive @web
    Scenario: Web home navigation uses capability discovery
      Given alice has signed up with email "alice-perms-capability-web@example.com" and password "p4ssw0rd"
      When alice discovers their self-permissions capability
      Then the capability indicates my permissions navigation is available
      And alice sees the My permissions navigation link on the home page

    @falsification @web
    Scenario: Web capability discovery denies another account id
      Given alice has signed up with email "alice-perms-capability-web-deny@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-perms-capability-web-deny@example.com" and password "p4ssw0rd"
      When alice attempts to discover bob's permissions capability through the self capability view
      Then the request fails with code "permission_denied"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI shows my mixed permission sources and policy constraint
      Given alice has signed up with email "alice-perms-tui@example.com" and password "p4ssw0rd"
      And alice has direct and role-template permissions with organization policy reason "Organization policy requires approval ticket."
      When alice views their own permissions
      Then alice sees organization and project permission sections
      And alice sees the direct organization permission entry
      And alice sees the role-template project permission entry
      And alice sees constrained permission reason "Organization policy requires approval ticket." from source "organization_policy"
      And the permissions view does not create permission request or grant events

    @falsification @tui
    Scenario: TUI requires authentication for self permissions
      When an unauthenticated client views their own permissions
      Then the request fails with code "auth_required"
      And the permissions view does not create permission request or grant events

    @falsification @tui
    Scenario: TUI denies requesting another account through the self view
      Given alice has signed up with email "alice-perms-tui-deny@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-perms-tui-deny@example.com" and password "p4ssw0rd"
      When alice attempts to view bob's permissions through the self view
      Then the request fails with code "permission_denied"
      And the permissions view does not create permission request or grant events
