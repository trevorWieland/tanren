@B-0137
Feature: Choose a deployment posture
  A user with permission can choose a deployment posture for a scope and
  Tanren explains what capabilities are available or unavailable.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API lists, sets, records, and attributes deployment posture changes
      Given an API account actor with posture permission
      When the actor lists supported deployment postures over API
      Then the API supported posture list includes capability summaries
      When the actor sets deployment posture "self_hosted" for their account scope over API
      Then the API response shows posture "self_hosted"
      And the API response includes available and unavailable capability summaries
      And the recorded posture for the actor account over API is "self_hosted"
      When the actor sets deployment posture "local_only" for their account scope over API
      Then the API response shows posture "local_only"
      And recent events attribute the posture change to the actor with posture "local_only"
      And the API response reflects inherited runtime and credential capability availability for posture "local_only"

    @falsification @api
    Scenario: API denies changing another account's posture
      Given a API account actor without posture permission
      When the actor sets deployment posture "hosted" for another account scope over API
      Then the request fails with code "permission_denied"
      And the error summary is readable

    @falsification @api
    Scenario: API rejects an unsupported deployment posture value
      Given an API account actor with posture permission
      When the actor sets deployment posture "unsupported-value" for their account scope over API
      Then the request fails with code "unsupported_posture"
      And the error summary is readable

    @falsification @api
    Scenario: API rejects a missing account scope
      Given an API account actor with posture permission
      When the actor sets deployment posture "hosted" for a missing account scope over API
      Then the request fails with code "scope_not_found"
      And the error summary is readable

  Rule: Web surface

    @positive @web
    Scenario: Web lists, sets, records, and attributes deployment posture changes
      Given a web account actor with posture permission
      When the actor lists supported deployment postures over web
      Then the web supported posture list includes capability summaries
      When the actor sets deployment posture "hosted" for their account scope over web
      Then the web view shows posture "hosted"
      And the web view shows available and unavailable capability summaries
      And the recorded posture for the actor account over web is "hosted"
      When the actor sets deployment posture "self_hosted" for their account scope over web
      Then the web view shows posture "self_hosted"
      And recent events attribute the posture change to the actor with posture "self_hosted"
      And the web view reflects inherited runtime and credential capability availability for posture "self_hosted"

    @falsification @web
    Scenario: Web denies changing another account's posture
      Given a web account actor without posture permission
      When the actor sets deployment posture "hosted" for another account scope over web
      Then the request fails with code "permission_denied"
      And the error summary is readable

    @falsification @web
    Scenario: Web rejects an unsupported deployment posture value
      Given a web account actor with posture permission
      When the actor sets deployment posture "unsupported-value" for their account scope over web
      Then the request fails with code "unsupported_posture"
      And the error summary is readable

    @falsification @web
    Scenario: Web rejects a missing account scope
      Given a web account actor with posture permission
      When the actor sets deployment posture "hosted" for a missing account scope over web
      Then the request fails with code "scope_not_found"
      And the error summary is readable

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP lists, sets, records, and attributes deployment posture changes
      Given an MCP account actor with posture permission
      When the actor lists supported deployment postures over MCP
      Then the MCP supported posture list includes capability summaries
      When the actor sets deployment posture "local_only" for their account scope over MCP
      Then the MCP response shows posture "local_only"
      And the MCP response includes available and unavailable capability summaries
      And the recorded posture for the actor account over MCP is "local_only"
      When the actor sets deployment posture "hosted" for their account scope over MCP
      Then the MCP response shows posture "hosted"
      And recent events attribute the posture change to the actor with posture "hosted"
      And the MCP response reflects inherited runtime and credential capability availability for posture "hosted"

    @falsification @mcp
    Scenario: MCP denies changing another account's posture
      Given an MCP account actor without posture permission
      When the actor sets deployment posture "hosted" for another account scope over MCP
      Then the request fails with code "permission_denied"
      And the error summary is readable

    @falsification @mcp
    Scenario: MCP rejects an unsupported deployment posture value
      Given an MCP account actor with posture permission
      When the actor sets deployment posture "unsupported-value" for their account scope over MCP
      Then the request fails with code "unsupported_posture"
      And the error summary is readable

    @falsification @mcp
    Scenario: MCP rejects a missing account scope
      Given an MCP account actor with posture permission
      When the actor sets deployment posture "hosted" for a missing account scope over MCP
      Then the request fails with code "scope_not_found"
      And the error summary is readable

  Rule: CLI surface

    @positive @cli
    Scenario: CLI lists, sets, records, and attributes deployment posture changes
      Given a CLI account actor with posture permission
      When the actor lists supported deployment postures over CLI
      Then the CLI supported posture list includes capability summaries
      When the actor sets deployment posture "self_hosted" for their account scope over CLI
      Then the CLI output shows posture "self_hosted"
      And the CLI output includes available and unavailable capability summaries
      And the recorded posture for the actor account over CLI is "self_hosted"
      When the actor sets deployment posture "hosted" for their account scope over CLI
      Then the CLI output shows posture "hosted"
      And recent events attribute the posture change to the actor with posture "hosted"
      And the CLI output reflects inherited runtime and credential capability availability for posture "hosted"

    @falsification @cli
    Scenario: CLI denies changing another account's posture
      Given a CLI account actor without posture permission
      When the actor sets deployment posture "hosted" for another account scope over CLI
      Then the request fails with code "permission_denied"
      And the error summary is readable

    @falsification @cli
    Scenario: CLI rejects an unsupported deployment posture value
      Given a CLI account actor with posture permission
      When the actor sets deployment posture "unsupported-value" for their account scope over CLI
      Then the request fails with code "unsupported_posture"
      And the error summary is readable

    @falsification @cli
    Scenario: CLI rejects a missing account scope
      Given a CLI account actor with posture permission
      When the actor sets deployment posture "hosted" for a missing account scope over CLI
      Then the request fails with code "scope_not_found"
      And the error summary is readable

  Rule: TUI surface

    @positive @tui
    Scenario: TUI lists, sets, records, and attributes deployment posture changes
      Given a TUI account actor with posture permission
      When the actor lists supported deployment postures over TUI
      Then the TUI supported posture list includes capability summaries
      When the actor sets deployment posture "hosted" for their account scope over TUI
      Then the TUI output shows posture "hosted"
      And the TUI output includes available and unavailable capability summaries
      And the recorded posture for the actor account over TUI is "hosted"
      When the actor sets deployment posture "local_only" for their account scope over TUI
      Then the TUI output shows posture "local_only"
      And recent events attribute the posture change to the actor with posture "local_only"
      And the TUI output reflects inherited runtime and credential capability availability for posture "local_only"

    @falsification @tui
    Scenario: TUI denies changing another account's posture
      Given a TUI account actor without posture permission
      When the actor sets deployment posture "hosted" for another account scope over TUI
      Then the request fails with code "permission_denied"
      And the error summary is readable

    @falsification @tui
    Scenario: TUI rejects an unsupported deployment posture value
      Given a TUI account actor with posture permission
      When the actor sets deployment posture "unsupported-value" for their account scope over TUI
      Then the request fails with code "unsupported_posture"
      And the error summary is readable

    @falsification @tui
    Scenario: TUI rejects a missing account scope
      Given a TUI account actor with posture permission
      When the actor sets deployment posture "hosted" for a missing account scope over TUI
      Then the request fails with code "scope_not_found"
      And the error summary is readable
