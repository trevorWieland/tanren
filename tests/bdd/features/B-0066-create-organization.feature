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
      Then alice is a member of "Alpha-API-Org"
      And the organization "Alpha-API-Org" has 0 projects
      And the organization "Alpha-API-Org" appears in alice's available organizations
      And alice holds all bootstrap admin permissions for "Alpha-API-Org"
      And a creator admin probe for invite passes on "Alpha-API-Org"
      And a creator admin probe for manage_access passes on "Alpha-API-Org"
      And a creator admin probe for configure passes on "Alpha-API-Org"
      And a creator admin probe for set_policy passes on "Alpha-API-Org"
      And a creator admin probe for delete passes on "Alpha-API-Org"
      And a "organization_created" event is recorded

    @falsification @api
    Scenario: API rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-API-Org"
      Then the request fails with code "auth_required"
      And a "organization_creation_rejected" event is recorded

    @falsification @api
    Scenario: API rejects creating an organization with a duplicate name
      Given alice has signed up with email "alice-org-dup-api@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Dup-API-Org"
      And alice creates an organization named "Dup-API-Org"
      Then the request fails with code "duplicate_organization_name"
      And a "organization_creation_rejected" event is recorded

    @falsification @api
    Scenario: API non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-api@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-API-Org"
      And bob has signed up with email "bob-org-admin-api@example.com" and password "p4ssw0rd"
      When bob attempts to invite members to "Admin-API-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to manage access to "Admin-API-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to configure "Admin-API-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to set policy for "Admin-API-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to delete "Admin-API-Org"
      Then the request fails with code "permission_denied"

  Rule: Web surface

    @positive @web
    Scenario: Signed-in web user creates an organization
      Given alice has signed up with email "alice-org-web@example.com" and password "p4ssw0rd"
      And alice signs in with the same credentials
      When alice creates an organization named "Alpha-Web-Org"
      Then alice is a member of "Alpha-Web-Org"
      And the organization "Alpha-Web-Org" has 0 projects
      And the organization "Alpha-Web-Org" appears in alice's available organizations
      And alice holds all bootstrap admin permissions for "Alpha-Web-Org"
      And a creator admin probe for invite passes on "Alpha-Web-Org"
      And a creator admin probe for manage_access passes on "Alpha-Web-Org"
      And a creator admin probe for configure passes on "Alpha-Web-Org"
      And a creator admin probe for set_policy passes on "Alpha-Web-Org"
      And a creator admin probe for delete passes on "Alpha-Web-Org"

    @falsification @web
    Scenario: Web rejects organization creation without authentication
      When guest creates an organization named "Ghost-Web-Org"
      Then the request fails with code "auth_required"

    @falsification @web
    Scenario: Web rejects creating an organization with a duplicate name
      Given alice has signed up with email "alice-org-dup-web@example.com" and password "p4ssw0rd"
      And alice signs in with the same credentials
      When alice creates an organization named "Dup-Web-Org"
      And alice creates an organization named "Dup-Web-Org"
      Then the request fails with code "duplicate_organization_name"

    @falsification @web
    Scenario: Web non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-web@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-Web-Org"
      And bob has signed up with email "bob-org-admin-web@example.com" and password "p4ssw0rd"
      When bob attempts to invite members to "Admin-Web-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to manage access to "Admin-Web-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to configure "Admin-Web-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to set policy for "Admin-Web-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to delete "Admin-Web-Org"
      Then the request fails with code "permission_denied"

  Rule: CLI surface

    @positive @cli
    Scenario: Signed-in CLI user creates an organization
      Given alice has signed up with email "alice-org-cli@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-CLI-Org"
      Then alice is a member of "Alpha-CLI-Org"
      And the organization "Alpha-CLI-Org" has 0 projects
      And the organization "Alpha-CLI-Org" appears in alice's available organizations
      And alice holds all bootstrap admin permissions for "Alpha-CLI-Org"
      And a creator admin probe for invite passes on "Alpha-CLI-Org"
      And a creator admin probe for manage_access passes on "Alpha-CLI-Org"
      And a creator admin probe for configure passes on "Alpha-CLI-Org"
      And a creator admin probe for set_policy passes on "Alpha-CLI-Org"
      And a creator admin probe for delete passes on "Alpha-CLI-Org"
      And a "organization_created" event is recorded

    @falsification @cli
    Scenario: CLI rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-CLI-Org"
      Then the request fails with code "auth_required"
      And a "organization_creation_rejected" event is recorded

    @falsification @cli
    Scenario: CLI rejects creating an organization with a duplicate name
      Given alice has signed up with email "alice-org-dup-cli@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Dup-CLI-Org"
      And alice creates an organization named "Dup-CLI-Org"
      Then the request fails with code "duplicate_organization_name"
      And a "organization_creation_rejected" event is recorded

    @falsification @cli
    Scenario: CLI non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-cli@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-CLI-Org"
      And bob has signed up with email "bob-org-admin-cli@example.com" and password "p4ssw0rd"
      When bob attempts to invite members to "Admin-CLI-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to manage access to "Admin-CLI-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to configure "Admin-CLI-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to set policy for "Admin-CLI-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to delete "Admin-CLI-Org"
      Then the request fails with code "permission_denied"

  Rule: MCP surface

    @positive @mcp
    Scenario: Signed-in MCP user creates an organization
      Given alice has signed up with email "alice-org-mcp@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-MCP-Org"
      Then alice is a member of "Alpha-MCP-Org"
      And the organization "Alpha-MCP-Org" has 0 projects
      And the organization "Alpha-MCP-Org" appears in alice's available organizations
      And alice holds all bootstrap admin permissions for "Alpha-MCP-Org"
      And a creator admin probe for invite passes on "Alpha-MCP-Org"
      And a creator admin probe for manage_access passes on "Alpha-MCP-Org"
      And a creator admin probe for configure passes on "Alpha-MCP-Org"
      And a creator admin probe for set_policy passes on "Alpha-MCP-Org"
      And a creator admin probe for delete passes on "Alpha-MCP-Org"
      And a "organization_created" event is recorded

    @falsification @mcp
    Scenario: MCP rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-MCP-Org"
      Then the request fails with code "auth_required"
      And a "organization_creation_rejected" event is recorded

    @falsification @mcp
    Scenario: MCP rejects creating an organization with a duplicate name
      Given alice has signed up with email "alice-org-dup-mcp@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Dup-MCP-Org"
      And alice creates an organization named "Dup-MCP-Org"
      Then the request fails with code "duplicate_organization_name"
      And a "organization_creation_rejected" event is recorded

    @falsification @mcp
    Scenario: MCP non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-mcp@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-MCP-Org"
      And bob has signed up with email "bob-org-admin-mcp@example.com" and password "p4ssw0rd"
      When bob attempts to invite members to "Admin-MCP-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to manage access to "Admin-MCP-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to configure "Admin-MCP-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to set policy for "Admin-MCP-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to delete "Admin-MCP-Org"
      Then the request fails with code "permission_denied"

  Rule: TUI surface

    @positive @tui
    Scenario: Signed-in TUI user creates an organization
      Given alice has signed up with email "alice-org-tui@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-TUI-Org"
      Then alice is a member of "Alpha-TUI-Org"
      And the organization "Alpha-TUI-Org" has 0 projects
      And the organization "Alpha-TUI-Org" appears in alice's available organizations
      And alice holds all bootstrap admin permissions for "Alpha-TUI-Org"
      And a creator admin probe for invite passes on "Alpha-TUI-Org"
      And a creator admin probe for manage_access passes on "Alpha-TUI-Org"
      And a creator admin probe for configure passes on "Alpha-TUI-Org"
      And a creator admin probe for set_policy passes on "Alpha-TUI-Org"
      And a creator admin probe for delete passes on "Alpha-TUI-Org"
      And a "organization_created" event is recorded

    @falsification @tui
    Scenario: TUI rejects organization creation without authentication
      When an unauthenticated request creates an organization named "Ghost-TUI-Org"
      Then the request fails with code "auth_required"
      And a "organization_creation_rejected" event is recorded

    @falsification @tui
    Scenario: TUI rejects creating an organization with a duplicate name
      Given alice has signed up with email "alice-org-dup-tui@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Dup-TUI-Org"
      And alice creates an organization named "Dup-TUI-Org"
      Then the request fails with code "duplicate_organization_name"
      And a "organization_creation_rejected" event is recorded

    @falsification @tui
    Scenario: TUI non-creator cannot invoke admin operations on an organization
      Given alice has signed up with email "alice-org-admin-tui@example.com" and password "p4ssw0rd"
      And alice creates an organization named "Admin-TUI-Org"
      And bob has signed up with email "bob-org-admin-tui@example.com" and password "p4ssw0rd"
      When bob attempts to invite members to "Admin-TUI-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to manage access to "Admin-TUI-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to configure "Admin-TUI-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to set policy for "Admin-TUI-Org"
      Then the request fails with code "permission_denied"
      When bob attempts to delete "Admin-TUI-Org"
      Then the request fails with code "permission_denied"
