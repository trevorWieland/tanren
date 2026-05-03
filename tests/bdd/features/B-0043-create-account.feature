@B-0043
Feature: Create an account
  A person can create a Tanren account, either by self-signup for a
  personal account or by accepting an invitation from an existing
  organization member, and then sign in to use Tanren. Each interface
  in B-0043 (`web`, `api`, `mcp`, `cli`, `tui`) repeats the same seven
  witnesses required by the spec's expected_evidence: four positive
  shapes (self-signup, invitation acceptance, multi-account, personal
  no-org) and three falsification shapes (duplicate identifier, wrong
  credential, expired invitation). The interface tag is a witness
  label — every surface routes through the same `Handlers` facade per
  the equivalent-operations rule in interfaces.md.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: Self-signup over the API creates an account that can sign in
      When alice self-signs up with email "alice-api@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      And a "account_created" event is recorded
      When alice signs in with the same credentials
      Then alice receives a session token
      And a "signed_in" event is recorded

    @positive @api
    Scenario: Invitation acceptance over the API joins the inviting org
      Given a pending invitation token "api-token-1-padpad"
      When bob accepts invitation "api-token-1-padpad" with password "team-pw"
      Then bob receives a session token
      And bob has joined an organization
      And a "invitation_accepted" event is recorded

    @positive @api
    Scenario: One person holds two accounts via the API
      When alice self-signs up with email "alice-api-multi@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      Given a pending invitation token "api-token-multi-padpad"
      When alice accepts invitation "api-token-multi-padpad" with password "work-pw"
      Then alice now holds 2 accounts

    @positive @api
    Scenario: Self-signed-up API account belongs to no organization
      When dave self-signs up with email "dave-api@example.com" and password "p4ssw0rd"
      Then dave receives a session token
      And dave's account belongs to no organization

    @falsification @api
    Scenario: API rejects sign-up with a duplicate identifier
      Given alice has signed up with email "alice-api-dup@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-api-dup@example.com" and password "different-pw"
      Then the request fails with code "duplicate_identifier"
      And a "sign_up_rejected" event is recorded

    @falsification @api
    Scenario: API rejects sign-up with a case variant of an existing identifier
      Given alice has signed up with email "alice-api-case@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "ALICE-API-CASE@Example.COM" and password "different-pw"
      Then the request fails with code "duplicate_identifier"
      And a "sign_up_rejected" event is recorded

    @falsification @api
    Scenario: API rejects sign-in with a wrong credential
      Given alice has signed up with email "alice-api-wrong@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-api-wrong@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"
      And a "sign_in_failed" event is recorded

    @falsification @api
    Scenario: API rejects accepting an expired invitation
      Given an expired invitation token "api-token-expired-padpad"
      When erin accepts invitation "api-token-expired-padpad" with password "any-pw"
      Then the request fails with code "invitation_expired"
      And a "invitation_accept_failed" event is recorded

    @falsification @api
    Scenario: API serializes concurrent acceptances of one invitation
      Given a pending invitation token "api-race-token-padpad"
      When 20 actors concurrently accept invitation "api-race-token-padpad"
      Then exactly 1 acceptance succeeds
      And 19 fail with code "invitation_already_consumed"

  Rule: Web surface

    @positive @web
    Scenario: Self-signup over the web creates an account that can sign in
      When alice self-signs up with email "alice-web@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice signs in with the same credentials
      Then alice receives a session token

    @positive @web
    Scenario: Invitation acceptance over the web joins the inviting org
      Given a pending invitation token "web-token-1-padpad"
      When bob accepts invitation "web-token-1-padpad" with password "team-pw"
      Then bob receives a session token
      And bob has joined an organization

    @positive @web
    Scenario: One person holds two accounts via the web
      When alice self-signs up with email "alice-web-multi@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      Given a pending invitation token "web-token-multi-padpad"
      When alice accepts invitation "web-token-multi-padpad" with password "work-pw"
      Then alice now holds 2 accounts

    @positive @web
    Scenario: Self-signed-up web account belongs to no organization
      When dave self-signs up with email "dave-web@example.com" and password "p4ssw0rd"
      Then dave receives a session token
      And dave's account belongs to no organization

    @falsification @web
    Scenario: Web rejects sign-up with a duplicate identifier
      Given alice has signed up with email "alice-web-dup@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-web-dup@example.com" and password "different-pw"
      Then the request fails with code "duplicate_identifier"

    @falsification @web
    Scenario: Web rejects sign-in with a wrong credential
      Given alice has signed up with email "alice-web-wrong@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-web-wrong@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"

    @falsification @web
    Scenario: Web rejects accepting an expired invitation
      Given an expired invitation token "web-token-expired-padpad"
      When erin accepts invitation "web-token-expired-padpad" with password "any-pw"
      Then the request fails with code "invitation_expired"

  Rule: CLI surface

    @positive @cli
    Scenario: Self-signup over the CLI creates an account that can sign in
      When alice self-signs up with email "alice-cli@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice signs in with the same credentials
      Then alice receives a session token

    @positive @cli
    Scenario: Invitation acceptance over the CLI joins the inviting org
      Given a pending invitation token "cli-token-1-padpad"
      When bob accepts invitation "cli-token-1-padpad" with password "team-pw"
      Then bob receives a session token
      And bob has joined an organization

    @positive @cli
    Scenario: One person holds two accounts via the CLI
      When alice self-signs up with email "alice-cli-multi@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      Given a pending invitation token "cli-token-multi-padpad"
      When alice accepts invitation "cli-token-multi-padpad" with password "work-pw"
      Then alice now holds 2 accounts

    @positive @cli
    Scenario: Self-signed-up CLI account belongs to no organization
      When dave self-signs up with email "dave-cli@example.com" and password "p4ssw0rd"
      Then dave receives a session token
      And dave's account belongs to no organization

    @falsification @cli
    Scenario: CLI rejects sign-up with a duplicate identifier
      Given alice has signed up with email "alice-cli-dup@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-cli-dup@example.com" and password "different-pw"
      Then the request fails with code "duplicate_identifier"

    @falsification @cli
    Scenario: CLI rejects sign-in with a wrong credential
      Given alice has signed up with email "alice-cli-wrong@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-cli-wrong@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"

    @falsification @cli
    Scenario: CLI rejects accepting an expired invitation
      Given an expired invitation token "cli-token-expired-padpad"
      When erin accepts invitation "cli-token-expired-padpad" with password "any-pw"
      Then the request fails with code "invitation_expired"

  Rule: MCP surface

    @positive @mcp
    Scenario: Self-signup over MCP creates an account that can sign in
      When alice self-signs up with email "alice-mcp@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice signs in with the same credentials
      Then alice receives a session token

    @positive @mcp
    Scenario: Invitation acceptance over MCP joins the inviting org
      Given a pending invitation token "mcp-token-1-padpad"
      When bob accepts invitation "mcp-token-1-padpad" with password "team-pw"
      Then bob receives a session token
      And bob has joined an organization

    @positive @mcp
    Scenario: One person holds two accounts via MCP
      When alice self-signs up with email "alice-mcp-multi@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      Given a pending invitation token "mcp-token-multi-padpad"
      When alice accepts invitation "mcp-token-multi-padpad" with password "work-pw"
      Then alice now holds 2 accounts

    @positive @mcp
    Scenario: Self-signed-up MCP account belongs to no organization
      When dave self-signs up with email "dave-mcp@example.com" and password "p4ssw0rd"
      Then dave receives a session token
      And dave's account belongs to no organization

    @falsification @mcp
    Scenario: MCP rejects sign-up with a duplicate identifier
      Given alice has signed up with email "alice-mcp-dup@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-mcp-dup@example.com" and password "different-pw"
      Then the request fails with code "duplicate_identifier"

    @falsification @mcp
    Scenario: MCP rejects sign-in with a wrong credential
      Given alice has signed up with email "alice-mcp-wrong@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-mcp-wrong@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"

    @falsification @mcp
    Scenario: MCP rejects accepting an expired invitation
      Given an expired invitation token "mcp-token-expired-padpad"
      When erin accepts invitation "mcp-token-expired-padpad" with password "any-pw"
      Then the request fails with code "invitation_expired"

  Rule: TUI surface

    @positive @tui
    Scenario: Self-signup over the TUI creates an account that can sign in
      When alice self-signs up with email "alice-tui@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice signs in with the same credentials
      Then alice receives a session token

    @positive @tui
    Scenario: Invitation acceptance over the TUI joins the inviting org
      Given a pending invitation token "tui-token-1-padpad"
      When frank accepts invitation "tui-token-1-padpad" with password "team-pw"
      Then frank receives a session token
      And frank has joined an organization

    @positive @tui
    Scenario: One person holds two accounts via the TUI
      When alice self-signs up with email "alice-tui-multi@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      Given a pending invitation token "tui-token-multi-padpad"
      When alice accepts invitation "tui-token-multi-padpad" with password "work-pw"
      Then alice now holds 2 accounts

    @positive @tui
    Scenario: Self-signed-up TUI account belongs to no organization
      When dave self-signs up with email "dave-tui@example.com" and password "p4ssw0rd"
      Then dave receives a session token
      And dave's account belongs to no organization

    @falsification @tui
    Scenario: TUI rejects sign-up with a duplicate identifier
      Given alice has signed up with email "alice-tui-dup@example.com" and password "p4ssw0rd"
      When mallory self-signs up with email "alice-tui-dup@example.com" and password "other-pw"
      Then the request fails with code "duplicate_identifier"

    @falsification @tui
    Scenario: TUI rejects sign-in with a wrong credential
      Given alice has signed up with email "alice-tui-wrong@example.com" and password "p4ssw0rd"
      When alice signs in with email "alice-tui-wrong@example.com" and password "wrong-pw"
      Then the request fails with code "invalid_credential"

    @falsification @tui
    Scenario: TUI rejects accepting an expired invitation
      Given an expired invitation token "tui-token-expired-padpad"
      When erin accepts invitation "tui-token-expired-padpad" with password "any-pw"
      Then the request fails with code "invitation_expired"
