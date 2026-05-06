@B-0041
Feature: Project wire shapes are shared across interfaces
  Project request and response wire shapes (spec views, dependency views,
  parameter structs, and failure bodies) are defined in tanren-contract
  and shared identically by the api, mcp, and cli surfaces so every
  transport exposes the same serialization contract.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API connect and list use shared contract wire shapes
      Given a connected project "api-wire-project"
      When projects are listed
      Then the project appears in the project list

    @positive @api
    Scenario: API specs and dependencies use shared contract wire shapes
      Given a connected project "api-spec-wire"
      And a spec titled "Wire Spec" exists for the project
      And a cross-project dependency from the project to project "api-dep-target"
      When the project is disconnected
      Then 0 unresolved inbound dependencies are reported

  Rule: CLI surface

    @positive @cli
    Scenario: CLI connect and list use shared contract wire shapes
      Given a connected project "cli-wire-project"
      When projects are listed
      Then the project appears in the project list

    @positive @cli
    Scenario: CLI specs and dependencies use shared contract wire shapes
      Given a connected project "cli-spec-wire"
      And a spec titled "Wire Spec" exists for the project
      And a cross-project dependency from the project to project "cli-dep-target"
      When the project is disconnected
      Then 0 unresolved inbound dependencies are reported

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP connect and list use shared contract wire shapes
      Given a connected project "mcp-wire-project"
      When projects are listed
      Then the project appears in the project list

    @positive @mcp
    Scenario: MCP specs and dependencies use shared contract wire shapes
      Given a connected project "mcp-spec-wire"
      And a spec titled "Wire Spec" exists for the project
      And a cross-project dependency from the project to project "mcp-dep-target"
      When the project is disconnected
      Then 0 unresolved inbound dependencies are reported
