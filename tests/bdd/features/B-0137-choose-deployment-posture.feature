@B-0137
Feature: Choose a deployment posture
  A user with posture-admin permission can select the deployment posture
  (hosted, self-hosted, local-only) for the installation. The posture is
  visible before project work is dispatched and gates first-run progress.
  Later runtime and credential choices inherit the selected posture unless
  changed with permission. Each interface in B-0137 (`web`, `api`, `mcp`,
  `cli`, `tui`) covers listing postures with capability summaries, selecting
  and verifying a posture, changing posture later, rejecting unpermitted
  users, and rejecting unsupported posture values.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: Admin lists postures with capabilities and selects local_only over API
      When the actor lists available postures
      Then the posture list contains 3 entries
      And the posture list includes "hosted"
      And the posture list includes "self_hosted"
      And the posture list includes "local_only"
      When the admin sets the posture to "local_only"
      Then the current posture is "local_only"
      And a "posture_set" event is recorded

    @positive @api
    Scenario: Admin changes posture from local_only to self_hosted over API
      When the admin sets the posture to "local_only"
      Then the current posture is "local_only"
      When the admin sets the posture to "self_hosted"
      Then the current posture is "self_hosted"
      And a "posture_set" event is recorded

    @falsification @api
    Scenario: API rejects posture change from non-admin
      When a non-admin sets the posture to "hosted"
      Then the posture request fails with code "permission_denied"

    @falsification @api
    Scenario: API rejects an unsupported posture value
      When the admin sets the posture to "bogus_posture"
      Then the posture request fails with code "unsupported_posture"

  Rule: Web surface

    @positive @web
    Scenario: Admin lists postures with capabilities and selects self_hosted over web
      When the actor lists available postures
      Then the posture list contains 3 entries
      And the posture list includes "hosted"
      And the posture list includes "self_hosted"
      And the posture list includes "local_only"
      When the admin sets the posture to "self_hosted"
      Then the current posture is "self_hosted"
      And a "posture_set" event is recorded

    @positive @web
    Scenario: Admin changes posture from hosted to local_only over web
      When the admin sets the posture to "hosted"
      Then the current posture is "hosted"
      When the admin sets the posture to "local_only"
      Then the current posture is "local_only"
      And a "posture_set" event is recorded

    @falsification @web
    Scenario: Web rejects posture change from non-admin
      When a non-admin sets the posture to "hosted"
      Then the posture request fails with code "permission_denied"

    @falsification @web
    Scenario: Web rejects an unsupported posture value
      When the admin sets the posture to "bogus_posture"
      Then the posture request fails with code "unsupported_posture"

  Rule: CLI surface

    @positive @cli
    Scenario: Admin lists postures with capabilities and selects hosted over CLI
      When the actor lists available postures
      Then the posture list contains 3 entries
      And the posture list includes "hosted"
      And the posture list includes "self_hosted"
      And the posture list includes "local_only"
      When the admin sets the posture to "hosted"
      Then the current posture is "hosted"
      And a "posture_set" event is recorded

    @positive @cli
    Scenario: Admin changes posture from local_only to hosted over CLI
      When the admin sets the posture to "local_only"
      Then the current posture is "local_only"
      When the admin sets the posture to "hosted"
      Then the current posture is "hosted"
      And a "posture_set" event is recorded

    @falsification @cli
    Scenario: CLI rejects posture change from non-admin
      When a non-admin sets the posture to "hosted"
      Then the posture request fails with code "permission_denied"

    @falsification @cli
    Scenario: CLI rejects an unsupported posture value
      When the admin sets the posture to "bogus_posture"
      Then the posture request fails with code "unsupported_posture"

  Rule: MCP surface

    @positive @mcp
    Scenario: Admin lists postures with capabilities and selects self_hosted over MCP
      When the actor lists available postures
      Then the posture list contains 3 entries
      And the posture list includes "hosted"
      And the posture list includes "self_hosted"
      And the posture list includes "local_only"
      When the admin sets the posture to "self_hosted"
      Then the current posture is "self_hosted"
      And a "posture_set" event is recorded

    @positive @mcp
    Scenario: Admin changes posture from hosted to local_only over MCP
      When the admin sets the posture to "hosted"
      Then the current posture is "hosted"
      When the admin sets the posture to "local_only"
      Then the current posture is "local_only"
      And a "posture_set" event is recorded

    @falsification @mcp
    Scenario: MCP rejects posture change from non-admin
      When a non-admin sets the posture to "hosted"
      Then the posture request fails with code "permission_denied"

    @falsification @mcp
    Scenario: MCP rejects an unsupported posture value
      When the admin sets the posture to "bogus_posture"
      Then the posture request fails with code "unsupported_posture"

  Rule: TUI surface

    @positive @tui
    Scenario: Admin lists postures with capabilities and selects local_only over TUI
      When the actor lists available postures
      Then the posture list contains 3 entries
      And the posture list includes "hosted"
      And the posture list includes "self_hosted"
      And the posture list includes "local_only"
      When the admin sets the posture to "local_only"
      Then the current posture is "local_only"
      And a "posture_set" event is recorded

    @positive @tui
    Scenario: Admin changes posture from self_hosted to hosted over TUI
      When the admin sets the posture to "self_hosted"
      Then the current posture is "self_hosted"
      When the admin sets the posture to "hosted"
      Then the current posture is "hosted"
      And a "posture_set" event is recorded

    @falsification @tui
    Scenario: TUI rejects posture change from non-admin
      When a non-admin sets the posture to "hosted"
      Then the posture request fails with code "permission_denied"

    @falsification @tui
    Scenario: TUI rejects an unsupported posture value
      When the admin sets the posture to "bogus_posture"
      Then the posture request fails with code "unsupported_posture"
