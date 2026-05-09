@B-0137
Feature: Choose a deployment posture
  A user with permission can choose a deployment posture for a scope and
  Tanren explains what capabilities are available or unavailable. Every
  interface in B-0137 (`web`, `api`, `mcp`, `cli`, `tui`) must prove one
  positive and one falsification witness.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API sets and reads deployment posture with capability summary
      Given an API account actor with posture permission
      When the actor sets deployment posture "self_hosted" for their account scope over API
      Then the API response shows posture "self_hosted"
      And the API response includes available and unavailable capability summaries

    @falsification @api
    Scenario: API rejects an unsupported deployment posture
      Given an API account actor with posture permission
      When the actor sets deployment posture "unsupported-value" for their account scope over API
      Then the request fails with code "unsupported_posture"
      And the error summary is readable

  Rule: Web surface

    @positive @web
    Scenario: Web sets and shows deployment posture with capability summary
      Given a web account actor with posture permission
      When the actor sets deployment posture "hosted" for their account scope over web
      Then the web view shows posture "hosted"
      And the web view shows available and unavailable capability summaries

    @falsification @web
    Scenario: Web denies posture changes without permission
      Given a web account actor without posture permission
      When the actor sets deployment posture "hosted" for another account scope over web
      Then the request fails with code "permission_denied"
      And the error summary is readable

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP sets and reads deployment posture with capability summary
      Given an MCP account actor with posture permission
      When the actor sets deployment posture "local_only" for their account scope over MCP
      Then the MCP response shows posture "local_only"
      And the MCP response includes available and unavailable capability summaries

    @falsification @mcp
    Scenario: MCP rejects an unsupported deployment posture
      Given an MCP account actor with posture permission
      When the actor sets deployment posture "unsupported-value" for their account scope over MCP
      Then the request fails with code "unsupported_posture"
      And the error summary is readable

  Rule: CLI surface

    @positive @cli
    Scenario: CLI sets and reads deployment posture with capability summary
      Given a CLI account actor with posture permission
      When the actor sets deployment posture "self_hosted" for their account scope over CLI
      Then the CLI output shows posture "self_hosted"
      And the CLI output includes available and unavailable capability summaries

    @falsification @cli
    Scenario: CLI denies posture changes without permission
      Given a CLI account actor without posture permission
      When the actor sets deployment posture "hosted" for another account scope over CLI
      Then the request fails with code "permission_denied"
      And the error summary is readable

  Rule: TUI surface

    @positive @tui
    Scenario: TUI sets and shows deployment posture with capability summary
      Given a TUI account actor with posture permission
      When the actor sets deployment posture "hosted" for their account scope over TUI
      Then the TUI output shows posture "hosted"
      And the TUI output shows available and unavailable capability summaries

    @falsification @tui
    Scenario: TUI rejects an unsupported deployment posture
      Given a TUI account actor with posture permission
      When the actor sets deployment posture "unsupported-value" for their account scope over TUI
      Then the request fails with code "unsupported_posture"
      And the error summary is readable
