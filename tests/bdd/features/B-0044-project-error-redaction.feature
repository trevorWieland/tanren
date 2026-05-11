@B-0044
Feature: Redact internal project errors on non-HTTP interfaces
  Project setup failures on CLI, MCP, and TUI surfaces expose only canonical
  failure codes and safe summaries.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API connects an existing repository without exposing internals
      Given alice has a project account
      And repository fixture "ApiRedact/Atlas" has fingerprint "repo-fp::apiredact/atlas" and 2 prior commits
      When alice connects existing repository "ApiRedact/Atlas" as an active project
      Then the connection succeeds

    @falsification @api
    Scenario: API reports duplicate repository with canonical code
      Given alice has a project account
      And repository fixture "ApiRedact/Single" has fingerprint "repo-fp::apiredact/single" and 2 prior commits
      When alice connects existing repository "ApiRedact/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "ApiRedact/Single" as an active project
      Then the project request fails with code "duplicate_repository"

  Rule: Web surface

    @positive @web
    Scenario: Web connects an existing repository without exposing internals
      Given alice has a project account
      And repository fixture "WebRedact/Atlas" has fingerprint "repo-fp::webredact/atlas" and 2 prior commits
      When alice connects existing repository "WebRedact/Atlas" as an active project
      Then the connection succeeds

    @falsification @web
    Scenario: Web reports duplicate repository with canonical code
      Given alice has a project account
      And repository fixture "WebRedact/Single" has fingerprint "repo-fp::webredact/single" and 2 prior commits
      When alice connects existing repository "WebRedact/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "WebRedact/Single" as an active project
      Then the project request fails with code "duplicate_repository"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI connects an existing repository without exposing internals
      Given alice has a project account
      And repository fixture "CliRedact/Atlas" has fingerprint "repo-fp::cliredact/atlas" and 2 prior commits
      When alice connects existing repository "CliRedact/Atlas" as an active project
      Then the connection succeeds

    @falsification @cli
    Scenario: CLI reports duplicate repository with canonical code
      Given alice has a project account
      And repository fixture "CliRedact/Single" has fingerprint "repo-fp::cliredact/single" and 2 prior commits
      When alice connects existing repository "CliRedact/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "CliRedact/Single" as an active project
      Then the project request fails with code "duplicate_repository"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP connects an existing repository without exposing internals
      Given alice has a project account
      And repository fixture "McpRedact/Atlas" has fingerprint "repo-fp::mcpredact/atlas" and 2 prior commits
      When alice connects existing repository "McpRedact/Atlas" as an active project
      Then the connection succeeds

    @falsification @mcp
    Scenario: MCP reports duplicate repository with canonical code
      Given alice has a project account
      And repository fixture "McpRedact/Single" has fingerprint "repo-fp::mcpredact/single" and 2 prior commits
      When alice connects existing repository "McpRedact/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "McpRedact/Single" as an active project
      Then the project request fails with code "duplicate_repository"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI connects an existing repository without exposing internals
      Given alice has a project account
      And repository fixture "TuiRedact/Atlas" has fingerprint "repo-fp::tuiredact/atlas" and 2 prior commits
      When alice connects existing repository "TuiRedact/Atlas" as an active project
      Then the connection succeeds

    @falsification @tui
    Scenario: TUI reports duplicate repository with canonical code
      Given alice has a project account
      And repository fixture "TuiRedact/Single" has fingerprint "repo-fp::tuiredact/single" and 2 prior commits
      When alice connects existing repository "TuiRedact/Single" as an active project
      Then the connection succeeds
      When alice connects existing repository "TuiRedact/Single" as an active project
      Then the project request fails with code "duplicate_repository"
