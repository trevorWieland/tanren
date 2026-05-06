@B-0047
Feature: Switch the active organization within an account
  A user whose active account belongs to more than one organization can
  list every organization and select any as the active one. Switching
  changes which projects are listed. Personal accounts (zero orgs)
  gracefully no-op. Each interface in B-0047 (`web`, `api`, `mcp`,
  `cli`, `tui`) provides positive and falsification witnesses.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: Switching active org over the API changes listed projects
      Given alice has signed up with email "alice-api-org@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"
      When alice switches active organization to "00000000-0000-0000-0000-000000000002"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000002"

    @positive @api
    Scenario: API lists all organizations the account belongs to
      Given alice has signed up with email "alice-api-list@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      When alice lists their organizations
      Then alice sees 2 organization memberships

    @positive @api
    Scenario: Personal API account sees no organization-scoped actions
      Given dave has signed up with email "dave-api@example.com" and password "p4ssw0rd"
      And dave is a personal account with zero organizations
      When dave lists their organizations
      Then dave sees the organization switcher is empty or disabled

    @falsification @api
    Scenario: API rejects switching to a non-member organization
      Given alice has signed up with email "alice-api-nm@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      And organization "00000000-0000-0000-0000-000000000003" has project "00000000-0000-0000-0000-000000000301" named "Gamma Project"
      When alice tries to switch active organization to "00000000-0000-0000-0000-000000000003"
      Then the request fails with code "organization-not-member"

    @falsification @api
    Scenario: API prevents project leakage after switching organizations
      Given alice has signed up with email "alice-api-leak@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"

  Rule: Web surface

    @positive @web
    Scenario: Switching active org over the web changes listed projects
      Given alice has signed up with email "alice-web-org@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"
      When alice switches active organization to "00000000-0000-0000-0000-000000000002"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000002"

    @positive @web
    Scenario: Web lists all organizations and displays the organization switcher
      Given alice has signed up with email "alice-web-list@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      When alice lists their organizations
      Then alice sees 2 organization memberships

    @positive @web
    Scenario: Personal web account sees no organization-scoped actions
      Given dave has signed up with email "dave-web@example.com" and password "p4ssw0rd"
      And dave is a personal account with zero organizations
      When dave lists their organizations
      Then dave sees the organization switcher is empty or disabled

    @falsification @web
    Scenario: Web rejects switching to a non-member organization
      Given alice has signed up with email "alice-web-nm@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      And organization "00000000-0000-0000-0000-000000000003" has project "00000000-0000-0000-0000-000000000301" named "Gamma Project"
      When alice tries to switch active organization to "00000000-0000-0000-0000-000000000003"
      Then the request fails with code "organization-not-member"

    @falsification @web
    Scenario: Web prevents project leakage after switching organizations
      Given alice has signed up with email "alice-web-leak@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"

  Rule: CLI surface

    @positive @cli
    Scenario: Switching active org over the CLI changes listed projects
      Given alice has signed up with email "alice-cli-org@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"
      When alice switches active organization to "00000000-0000-0000-0000-000000000002"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000002"

    @positive @cli
    Scenario: CLI lists all organizations the account belongs to
      Given alice has signed up with email "alice-cli-list@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      When alice lists their organizations
      Then alice sees 2 organization memberships

    @positive @cli
    Scenario: Personal CLI account sees no organization-scoped actions
      Given dave has signed up with email "dave-cli@example.com" and password "p4ssw0rd"
      And dave is a personal account with zero organizations
      When dave lists their organizations
      Then dave sees the organization switcher is empty or disabled

    @falsification @cli
    Scenario: CLI rejects switching to a non-member organization
      Given alice has signed up with email "alice-cli-nm@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      And organization "00000000-0000-0000-0000-000000000003" has project "00000000-0000-0000-0000-000000000301" named "Gamma Project"
      When alice tries to switch active organization to "00000000-0000-0000-0000-000000000003"
      Then the request fails with code "organization-not-member"

    @falsification @cli
    Scenario: CLI prevents project leakage after switching organizations
      Given alice has signed up with email "alice-cli-leak@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"

  Rule: MCP surface

    @positive @mcp
    Scenario: Switching active org over MCP changes listed projects
      Given alice has signed up with email "alice-mcp-org@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"
      When alice switches active organization to "00000000-0000-0000-0000-000000000002"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000002"

    @positive @mcp
    Scenario: MCP lists all organizations the account belongs to
      Given alice has signed up with email "alice-mcp-list@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      When alice lists their organizations
      Then alice sees 2 organization memberships

    @positive @mcp
    Scenario: Personal MCP account sees no organization-scoped actions
      Given dave has signed up with email "dave-mcp@example.com" and password "p4ssw0rd"
      And dave is a personal account with zero organizations
      When dave lists their organizations
      Then dave sees the organization switcher is empty or disabled

    @falsification @mcp
    Scenario: MCP rejects switching to a non-member organization
      Given alice has signed up with email "alice-mcp-nm@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      And organization "00000000-0000-0000-0000-000000000003" has project "00000000-0000-0000-0000-000000000301" named "Gamma Project"
      When alice tries to switch active organization to "00000000-0000-0000-0000-000000000003"
      Then the request fails with code "organization-not-member"

    @falsification @mcp
    Scenario: MCP prevents project leakage after switching organizations
      Given alice has signed up with email "alice-mcp-leak@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"

  Rule: TUI surface

    @positive @tui
    Scenario: Switching active org over the TUI changes listed projects
      Given alice has signed up with email "alice-tui-org@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"
      When alice switches active organization to "00000000-0000-0000-0000-000000000002"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000002"

    @positive @tui
    Scenario: TUI lists all organizations the account belongs to
      Given alice has signed up with email "alice-tui-list@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      When alice lists their organizations
      Then alice sees 2 organization memberships

    @positive @tui
    Scenario: Personal TUI account sees no organization-scoped actions
      Given dave has signed up with email "dave-tui@example.com" and password "p4ssw0rd"
      And dave is a personal account with zero organizations
      When dave lists their organizations
      Then dave sees the organization switcher is empty or disabled

    @falsification @tui
    Scenario: TUI rejects switching to a non-member organization
      Given alice has signed up with email "alice-tui-nm@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      And organization "00000000-0000-0000-0000-000000000003" has project "00000000-0000-0000-0000-000000000301" named "Gamma Project"
      When alice tries to switch active organization to "00000000-0000-0000-0000-000000000003"
      Then the request fails with code "organization-not-member"

    @falsification @tui
    Scenario: TUI prevents project leakage after switching organizations
      Given alice has signed up with email "alice-tui-leak@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" with project "00000000-0000-0000-0000-000000000101" named "Alpha Project" and organization "00000000-0000-0000-0000-000000000002" named "Beta" with project "00000000-0000-0000-0000-000000000102" named "Beta Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"
