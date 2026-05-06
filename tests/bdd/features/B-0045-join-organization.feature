@B-0045
Feature: Join an organization with an existing account
  A person with an existing Tanren account can accept an invitation
  to join an organization. The account gains org-level permissions
  from the invitation; other memberships are unaffected. Project
  access is NOT auto-granted. After acceptance the joined
  organization appears in the account's selectable organizations.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: Existing account joins an organization over the API with granted permissions
      Given alice has signed up with email "alice-join-api@example.com" and password "p4ssw0rd"
      And a pending invitation for "alice-join-api@example.com" with token "join-api-token-1-padpad" and "member" permissions
      When alice joins organization with invitation "join-api-token-1-padpad"
      Then alice is a member of the inviting organization
      And alice can select the inviting organization
      And alice has been granted "member" organization permissions
      And alice has no project access grants
      And alice is a member of 1 organizations

    @positive @api
    Scenario: Other org memberships are unaffected after API join
      Given alice has signed up with email "alice-multi-api@example.com" and password "p4ssw0rd"
      And alice is already a member of organization "existing-org-api"
      And a pending invitation for "alice-multi-api@example.com" with token "join-api-token-multi-padpad"
      When alice joins organization with invitation "join-api-token-multi-padpad"
      Then alice is a member of 2 organizations
      And a "organization_joined" event is recorded

    @falsification @api
    Scenario: API rejects joining with an invitation addressed to a different account
      Given alice has signed up with email "alice-wrong-api@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-wrong-api@example.com" and password "p4ssw0rd"
      And a pending invitation for "bob-wrong-api@example.com" with token "wrong-api-token-padpad"
      When alice joins organization with invitation "wrong-api-token-padpad"
      Then the request fails with code "wrong_account"
      And a "join_failed" event is recorded

    @falsification @api
    Scenario: API rejects joining with an expired invitation
      Given alice has signed up with email "alice-exp-api@example.com" and password "p4ssw0rd"
      And an expired invitation for "alice-exp-api@example.com" with token "exp-api-token-padpad"
      When alice joins organization with invitation "exp-api-token-padpad"
      Then the request fails with code "invitation_expired"
      And a "join_failed" event is recorded

    @falsification @api
    Scenario: API rejects joining with a revoked invitation
      Given alice has signed up with email "alice-rev-api@example.com" and password "p4ssw0rd"
      And a revoked invitation for "alice-rev-api@example.com" with token "rev-api-token-padpad"
      When alice joins organization with invitation "rev-api-token-padpad"
      Then the request fails with code "invitation_already_consumed"
      And a "join_failed" event is recorded

    @falsification @api
    Scenario: API rejects duplicate join of the same organization
      Given alice has signed up with email "alice-dup-api@example.com" and password "p4ssw0rd"
      And a pending invitation for "alice-dup-api@example.com" with token "dup-api-token-padpad"
      When alice joins organization with invitation "dup-api-token-padpad"
      Then alice is a member of the inviting organization
      And alice is a member of 1 organizations
      When alice joins organization with invitation "dup-api-token-padpad"
      Then the request fails with code "invitation_already_consumed"
      And alice is a member of 1 organizations

  Rule: Web surface

    @positive @web
    Scenario: Existing account joins an organization over the web
      Given alice has signed up with email "alice-join-web@example.com" and password "p4ssw0rd"
      And a pending invitation for "alice-join-web@example.com" with token "join-web-token-1-padpad"
      When alice joins organization with invitation "join-web-token-1-padpad"
      Then alice is a member of the inviting organization
      And alice can select the inviting organization
      And a "organization_joined" event is recorded
      And alice is a member of 1 organizations

    @positive @web
    Scenario: Other org memberships are unaffected after web join
      Given alice has signed up with email "alice-multi-web@example.com" and password "p4ssw0rd"
      And alice is already a member of organization "existing-org-web"
      And a pending invitation for "alice-multi-web@example.com" with token "join-web-token-multi-padpad"
      When alice joins organization with invitation "join-web-token-multi-padpad"
      Then alice is a member of 2 organizations

    @falsification @web
    Scenario: Web rejects joining with an invitation addressed to a different account
      Given alice has signed up with email "alice-wrong-web@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-wrong-web@example.com" and password "p4ssw0rd"
      And a pending invitation for "bob-wrong-web@example.com" with token "wrong-web-token-padpad"
      When alice joins organization with invitation "wrong-web-token-padpad"
      Then the request fails with code "wrong_account"
      And a "join_failed" event is recorded

    @falsification @web
    Scenario: Web rejects joining with an expired invitation
      Given alice has signed up with email "alice-exp-web@example.com" and password "p4ssw0rd"
      And an expired invitation for "alice-exp-web@example.com" with token "exp-web-token-padpad"
      When alice joins organization with invitation "exp-web-token-padpad"
      Then the request fails with code "invitation_expired"

  Rule: CLI surface

    @positive @cli
    Scenario: Existing account joins an organization over the CLI
      Given alice has signed up with email "alice-join-cli@example.com" and password "p4ssw0rd"
      And a pending invitation for "alice-join-cli@example.com" with token "join-cli-token-1-padpad"
      When alice joins organization with invitation "join-cli-token-1-padpad"
      Then alice is a member of the inviting organization
      And alice can select the inviting organization
      And a "organization_joined" event is recorded
      And alice is a member of 1 organizations

    @positive @cli
    Scenario: Other org memberships are unaffected after CLI join
      Given alice has signed up with email "alice-multi-cli@example.com" and password "p4ssw0rd"
      And alice is already a member of organization "existing-org-cli"
      And a pending invitation for "alice-multi-cli@example.com" with token "join-cli-token-multi-padpad"
      When alice joins organization with invitation "join-cli-token-multi-padpad"
      Then alice is a member of 2 organizations

    @falsification @cli
    Scenario: CLI rejects joining with an invitation addressed to a different account
      Given alice has signed up with email "alice-wrong-cli@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-wrong-cli@example.com" and password "p4ssw0rd"
      And a pending invitation for "bob-wrong-cli@example.com" with token "wrong-cli-token-padpad"
      When alice joins organization with invitation "wrong-cli-token-padpad"
      Then the request fails with code "wrong_account"
      And a "join_failed" event is recorded

    @falsification @cli
    Scenario: CLI rejects joining with an expired invitation
      Given alice has signed up with email "alice-exp-cli@example.com" and password "p4ssw0rd"
      And an expired invitation for "alice-exp-cli@example.com" with token "exp-cli-token-padpad"
      When alice joins organization with invitation "exp-cli-token-padpad"
      Then the request fails with code "invitation_expired"

  Rule: MCP surface

    @positive @mcp
    Scenario: Existing account joins an organization over MCP
      Given alice has signed up with email "alice-join-mcp@example.com" and password "p4ssw0rd"
      And a pending invitation for "alice-join-mcp@example.com" with token "join-mcp-token-1-padpad"
      When alice joins organization with invitation "join-mcp-token-1-padpad"
      Then alice is a member of the inviting organization
      And alice can select the inviting organization
      And a "organization_joined" event is recorded
      And alice is a member of 1 organizations

    @positive @mcp
    Scenario: Other org memberships are unaffected after MCP join
      Given alice has signed up with email "alice-multi-mcp@example.com" and password "p4ssw0rd"
      And alice is already a member of organization "existing-org-mcp"
      And a pending invitation for "alice-multi-mcp@example.com" with token "join-mcp-token-multi-padpad"
      When alice joins organization with invitation "join-mcp-token-multi-padpad"
      Then alice is a member of 2 organizations

    @falsification @mcp
    Scenario: MCP rejects joining with an invitation addressed to a different account
      Given alice has signed up with email "alice-wrong-mcp@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-wrong-mcp@example.com" and password "p4ssw0rd"
      And a pending invitation for "bob-wrong-mcp@example.com" with token "wrong-mcp-token-padpad"
      When alice joins organization with invitation "wrong-mcp-token-padpad"
      Then the request fails with code "wrong_account"
      And a "join_failed" event is recorded

    @falsification @mcp
    Scenario: MCP rejects joining with an expired invitation
      Given alice has signed up with email "alice-exp-mcp@example.com" and password "p4ssw0rd"
      And an expired invitation for "alice-exp-mcp@example.com" with token "exp-mcp-token-padpad"
      When alice joins organization with invitation "exp-mcp-token-padpad"
      Then the request fails with code "invitation_expired"

  Rule: TUI surface

    @positive @tui
    Scenario: Existing account joins an organization over the TUI
      Given alice has signed up with email "alice-join-tui@example.com" and password "p4ssw0rd"
      And a pending invitation for "alice-join-tui@example.com" with token "join-tui-token-1-padpad"
      When alice joins organization with invitation "join-tui-token-1-padpad"
      Then alice is a member of the inviting organization
      And alice can select the inviting organization
      And a "organization_joined" event is recorded
      And alice is a member of 1 organizations

    @positive @tui
    Scenario: Other org memberships are unaffected after TUI join
      Given alice has signed up with email "alice-multi-tui@example.com" and password "p4ssw0rd"
      And alice is already a member of organization "existing-org-tui"
      And a pending invitation for "alice-multi-tui@example.com" with token "join-tui-token-multi-padpad"
      When alice joins organization with invitation "join-tui-token-multi-padpad"
      Then alice is a member of 2 organizations

    @falsification @tui
    Scenario: TUI rejects joining with an invitation addressed to a different account
      Given alice has signed up with email "alice-wrong-tui@example.com" and password "p4ssw0rd"
      And bob has signed up with email "bob-wrong-tui@example.com" and password "p4ssw0rd"
      And a pending invitation for "bob-wrong-tui@example.com" with token "wrong-tui-token-padpad"
      When alice joins organization with invitation "wrong-tui-token-padpad"
      Then the request fails with code "wrong_account"
      And a "join_failed" event is recorded

    @falsification @tui
    Scenario: TUI rejects joining with an expired invitation
      Given alice has signed up with email "alice-exp-tui@example.com" and password "p4ssw0rd"
      And an expired invitation for "alice-exp-tui@example.com" with token "exp-tui-token-padpad"
      When alice joins organization with invitation "exp-tui-token-padpad"
      Then the request fails with code "invitation_expired"
