@B-0066
Feature: Create an organization
  A signed-in user can create a new organization and becomes its initial
  member with full bootstrap administrative permissions. The new
  organization appears in the creator's account and initially owns no
  projects. Each interface in B-0066 (`web`, `api`, `mcp`, `cli`, `tui`)
  witnesses: one positive path (org exists + creator holds full bootstrap
  permissions) and two falsification paths (unsigned-in creation rejected,
  different account has empty admin permissions).

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API org creation grants full bootstrap permissions and lists the org
      Given alice has signed up with email "alice-api@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Acme-api"
      Then the response includes full bootstrap permissions
      And "Acme-api" appears in alice's organization list
      And a "organization_created" event is recorded

    @falsification @api
    Scenario: API rejects unsigned-in organization creation
      When an unsigned-in attempt creates an organization named "Ghost-api"
      Then the request fails with code "unauthenticated"

    @positive @api
    Scenario: API invitation-accepted account tracks current identity for admin permission checks
      Given alice has signed up with email "alice-api-inv@example.com" and password "p4ssw0rd"
      And alice has created an organization named "Acme-api-inv"
      And a pending invitation token "api-inv-org-token-padpad"
      When bob accepts invitation "api-inv-org-token-padpad" with password "team-pw"
      Then bob's admin permissions on "Acme-api-inv" are empty

    @falsification @api
    Scenario: API other account has empty admin permissions on a freshly created org
      Given alice has signed up with email "alice-api-other@example.com" and password "p4ssw0rd"
      And alice has created an organization named "Acme-api-other"
      When bob self-signs up with email "bob-api-other@example.com" and password "p4ssw0rd"
      Then bob's admin permissions on "Acme-api-other" are empty

  Rule: Web surface

    @positive @web
    Scenario: Web org creation grants full bootstrap permissions and lists the org
      Given alice has signed up with email "alice-web-66@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Acme-web"
      Then the response includes full bootstrap permissions
      And "Acme-web" appears in alice's organization list

    @falsification @web
    Scenario: Web rejects unsigned-in organization creation
      When an unsigned-in attempt creates an organization named "Ghost-web"
      Then the request fails with code "unauthenticated"

    @falsification @web
    Scenario: Web other account has empty admin permissions on a freshly created org
      Given alice has signed up with email "alice-web-66a@example.com" and password "p4ssw0rd"
      And alice has created an organization named "Acme-web-other"
      When bob self-signs up with email "bob-web-66b@example.com" and password "p4ssw0rd"
      Then bob's admin permissions on "Acme-web-other" are empty

  Rule: CLI surface

    @positive @cli
    Scenario: CLI org creation grants full bootstrap permissions and lists the org
      Given alice has signed up with email "alice-cli@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Acme-cli"
      Then the response includes full bootstrap permissions
      And "Acme-cli" appears in alice's organization list

    @falsification @cli
    Scenario: CLI rejects unsigned-in organization creation
      When an unsigned-in attempt creates an organization named "Ghost-cli"
      Then the request fails with code "unauthenticated"

    @positive @cli
    Scenario: CLI invitation-accepted account tracks current identity for admin permission checks
      Given alice has signed up with email "alice-cli-inv@example.com" and password "p4ssw0rd"
      And alice has created an organization named "Acme-cli-inv"
      And a pending invitation token "cli-inv-org-token-padpad"
      When bob accepts invitation "cli-inv-org-token-padpad" with password "team-pw"
      Then bob's admin permissions on "Acme-cli-inv" are empty

    @falsification @cli
    Scenario: CLI other account has empty admin permissions on a freshly created org
      Given alice has signed up with email "alice-cli-other@example.com" and password "p4ssw0rd"
      And alice has created an organization named "Acme-cli-other"
      When bob self-signs up with email "bob-cli-other@example.com" and password "p4ssw0rd"
      Then bob's admin permissions on "Acme-cli-other" are empty

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP org creation grants full bootstrap permissions and lists the org
      Given alice has signed up with email "alice-mcp@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Acme-mcp"
      Then the response includes full bootstrap permissions
      And "Acme-mcp" appears in alice's organization list

    @falsification @mcp
    Scenario: MCP rejects unsigned-in organization creation
      When an unsigned-in attempt creates an organization named "Ghost-mcp"
      Then the request fails with code "unauthenticated"

    @falsification @mcp
    Scenario: MCP other account has empty admin permissions on a freshly created org
      Given alice has signed up with email "alice-mcp-other@example.com" and password "p4ssw0rd"
      And alice has created an organization named "Acme-mcp-other"
      When bob self-signs up with email "bob-mcp-other@example.com" and password "p4ssw0rd"
      Then bob's admin permissions on "Acme-mcp-other" are empty

  Rule: TUI surface

    @positive @tui
    Scenario: TUI org creation grants full bootstrap permissions and lists the org
      Given alice has signed up with email "alice-tui@example.com" and password "p4ssw0rd"
      When alice creates an organization named "Acme-tui"
      Then the response includes full bootstrap permissions
      And "Acme-tui" appears in alice's organization list

    @falsification @tui
    Scenario: TUI rejects unsigned-in organization creation
      When an unsigned-in attempt creates an organization named "Ghost-tui"
      Then the request fails with code "unauthenticated"

    @falsification @tui
    Scenario: TUI other account has empty admin permissions on a freshly created org
      Given alice has signed up with email "alice-tui-other@example.com" and password "p4ssw0rd"
      And alice has created an organization named "Acme-tui-other"
      When bob self-signs up with email "bob-tui-other@example.com" and password "p4ssw0rd"
      Then bob's admin permissions on "Acme-tui-other" are empty
