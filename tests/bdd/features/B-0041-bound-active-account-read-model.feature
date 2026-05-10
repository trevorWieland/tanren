@B-0041
Feature: Bound active-account read models
  The active-account list and switch read path is bounded so that a
  caller with many signed-in sessions cannot cause unbounded server or
  client resource use. Window context is resolved once per request
  lifecycle and reused until explicit rotation.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API returns a bounded active-account list
      Given alice holds two signed-in accounts via the api
      When alice lists active accounts via the api
      Then alice receives at most 16 signed-in accounts via the api

  Rule: Web surface

    @positive @web
    Scenario: Web returns a bounded active-account list
      Given alice holds two signed-in accounts via the web
      When alice lists active accounts via the web
      Then alice receives at most 16 signed-in accounts via the web

  Rule: CLI surface

    @positive @cli
    Scenario: CLI returns a bounded active-account list
      Given alice holds two signed-in accounts via the cli
      When alice lists active accounts via the cli
      Then alice receives at most 16 signed-in accounts via the cli

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP returns a bounded active-account list
      Given alice holds two signed-in accounts via the mcp
      When alice lists active accounts via the mcp
      Then alice receives at most 16 signed-in accounts via the mcp

  Rule: TUI surface

    @positive @tui
    Scenario: TUI returns a bounded active-account list
      Given alice holds two signed-in accounts via the tui
      When alice lists active accounts via the tui
      Then alice receives at most 16 signed-in accounts via the tui
