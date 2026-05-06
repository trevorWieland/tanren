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
      Then alice receives a session token
      And alice sees the organization "Alpha-API-Org" in her account
      And alice holds all bootstrap admin permissions for "Alpha-API-Org"
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

  Rule: Web surface

    @positive @web
    Scenario: Signed-in web user creates an organization
      Given alice has signed up with email "alice-org-web@example.com" and password "p4ssw0rd"
      And alice signs in with the same credentials
      When alice creates an organization named "Alpha-Web-Org"
      Then alice sees the organization "Alpha-Web-Org" listed
      And alice holds all bootstrap admin permissions for "Alpha-Web-Org"

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

  Rule: CLI surface

    @positive @cli
    Scenario: Signed-in CLI user creates an organization
      Given alice has signed up with email "alice-org-cli@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-CLI-Org"
      Then alice receives a session token
      And alice sees the organization "Alpha-CLI-Org" in her account
      And alice holds all bootstrap admin permissions for "Alpha-CLI-Org"
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

  Rule: MCP surface

    @positive @mcp
    Scenario: Signed-in MCP user creates an organization
      Given alice has signed up with email "alice-org-mcp@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-MCP-Org"
      Then alice receives a session token
      And alice sees the organization "Alpha-MCP-Org" in her account
      And alice holds all bootstrap admin permissions for "Alpha-MCP-Org"
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

  Rule: TUI surface

    @positive @tui
    Scenario: Signed-in TUI user creates an organization
      Given alice has signed up with email "alice-org-tui@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Alpha-TUI-Org"
      Then alice receives a session token
      And alice sees the organization "Alpha-TUI-Org" in her account
      And alice holds all bootstrap admin permissions for "Alpha-TUI-Org"
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
