@B-0066
Feature: Create an organization
  A signed-in user can create a new Tanren organization and
  become its initial member with bootstrap administrative
  permissions. Each interface in B-0066 repeats the same
  witnesses required by the spec's expected_evidence.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: Signed-in API user creates an organization
      Given alice has signed up with email "alice-org-api@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-API-Org"
      Then alice is a member of the organization
      And the organization has 0 projects
      And the organization appears in alice's available organizations
      And alice holds all bootstrap admin permissions
      And a "organization_created" event is recorded

    @falsification @api
    Scenario: API rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-API-Org"
      Then the request fails with code "auth_required"

    @falsification @api
    Scenario: API non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-api@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-API-Org"
      And bob has signed up with email "bob-org-admin-api@example.com" and password "p4ssw0rd"
      Then bob cannot authorize admin operation "invite_members" on the organization
      And bob cannot authorize admin operation "manage_access" on the organization
      And bob cannot authorize admin operation "configure" on the organization
      And bob cannot authorize admin operation "set_policy" on the organization
      And bob cannot authorize admin operation "delete" on the organization

  Rule: Web surface

    @positive @web
    Scenario: Signed-in web user creates an organization
      Given alice has signed up with email "alice-org-web@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice creates an organization named "Alpha-Web-Org"
      Then alice is a member of the organization
      And the organization has 0 projects
      And the organization appears in alice's available organizations
      And alice holds all bootstrap admin permissions

    @falsification @web
    Scenario: Web rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-Web-Org"
      Then the request fails with code "auth_required"

    @falsification @web
    Scenario: Web rejects creating an organization with a duplicate name
      Given alice has signed up with email "alice-org-dup-web@example.com" and password "p4ssw0rd"
      When alice signs in with the same credentials
      And alice creates an organization named "Dup-Web-Org"
      And alice creates an organization named "Dup-Web-Org"
      Then the request fails with code "duplicate_organization_name"

    @falsification @web
    Scenario: Web non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-web@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-Web-Org"
      And bob has signed up with email "bob-org-admin-web@example.com" and password "p4ssw0rd"
      Then bob cannot authorize admin operation "invite_members" on the organization
      And bob cannot authorize admin operation "manage_access" on the organization
      And bob cannot authorize admin operation "configure" on the organization
      And bob cannot authorize admin operation "set_policy" on the organization
      And bob cannot authorize admin operation "delete" on the organization

  Rule: CLI surface

    @positive @cli
    Scenario: Signed-in CLI user creates an organization
      Given alice has signed up with email "alice-org-cli@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-CLI-Org"
      Then alice is a member of the organization
      And the organization has 0 projects
      And the organization appears in alice's available organizations
      And alice holds all bootstrap admin permissions
      And a "organization_created" event is recorded

    @falsification @cli
    Scenario: CLI rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-CLI-Org"
      Then the request fails with code "auth_required"

    @falsification @cli
    Scenario: CLI non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-cli@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-CLI-Org"
      And bob has signed up with email "bob-org-admin-cli@example.com" and password "p4ssw0rd"
      Then bob cannot authorize admin operation "invite_members" on the organization
      And bob cannot authorize admin operation "manage_access" on the organization
      And bob cannot authorize admin operation "configure" on the organization
      And bob cannot authorize admin operation "set_policy" on the organization
      And bob cannot authorize admin operation "delete" on the organization

  Rule: MCP surface

    @positive @mcp
    Scenario: Signed-in MCP user creates an organization
      Given alice has signed up with email "alice-org-mcp@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-MCP-Org"
      Then alice is a member of the organization
      And the organization has 0 projects
      And the organization appears in alice's available organizations
      And alice holds all bootstrap admin permissions
      And a "organization_created" event is recorded

    @falsification @mcp
    Scenario: MCP rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-MCP-Org"
      Then the request fails with code "auth_required"

    @falsification @mcp
    Scenario: MCP non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-mcp@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-MCP-Org"
      And bob has signed up with email "bob-org-admin-mcp@example.com" and password "p4ssw0rd"
      Then bob cannot authorize admin operation "invite_members" on the organization
      And bob cannot authorize admin operation "manage_access" on the organization
      And bob cannot authorize admin operation "configure" on the organization
      And bob cannot authorize admin operation "set_policy" on the organization
      And bob cannot authorize admin operation "delete" on the organization

  Rule: TUI surface

    @positive @tui
    Scenario: Signed-in TUI user creates an organization
      Given alice has signed up with email "alice-org-tui@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-TUI-Org"
      Then alice is a member of the organization
      And the organization has 0 projects
      And the organization appears in alice's available organizations
      And alice holds all bootstrap admin permissions
      And a "organization_created" event is recorded

    @falsification @tui
    Scenario: TUI rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-TUI-Org"
      Then the request fails with code "auth_required"

    @falsification @tui
    Scenario: TUI non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-tui@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-TUI-Org"
      And bob has signed up with email "bob-org-admin-tui@example.com" and password "p4ssw0rd"
      Then bob cannot authorize admin operation "invite_members" on the organization
      And bob cannot authorize admin operation "manage_access" on the organization
      And bob cannot authorize admin operation "configure" on the organization
      And bob cannot authorize admin operation "set_policy" on the organization
      And bob cannot authorize admin operation "delete" on the organization
