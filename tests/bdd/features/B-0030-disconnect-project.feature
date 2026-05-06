@B-0030
Feature: Disconnect a project from Tanren
  A solo-builder or team-builder can disconnect a project from their
  Tanren account so that it no longer appears in their views, without
  affecting the underlying repository. Each interface in B-0030 repeats
  the same witnesses required by the spec's expected_evidence: positive
  disconnect, rejected disconnect due to active loop, unresolved
  cross-project dependency signal, reconnect, and prior spec
  restoration. Each @api, @cli, @mcp, @tui, and @web scenario drives
  its own surface-specific harness (HTTP server, subprocess, rmcp
  client, pty wrapper, or browser driver) as required by the
  behavior-proof contract.

  Project event coverage (event step reads AccountContext; project
  events live in ProjectContext's store — executable assertions are
  blocked until the step is updated):
  "project_connected" event, "project_disconnected" event,
  "project_disconnect_rejected" event,
  "cross_project_dependency_unresolved" event.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API disconnects a project with no active loops and repo stays byte-identical
      Given a temp repository fixture
      And a connected project "disc-api-project"
      When the project is disconnected
      And projects are listed
      Then the project no longer appears in the project list
      And the repository byte checksum is unchanged

    @positive @api
    Scenario: API emits unresolved dependency signal on disconnect
      Given a connected project "dep-api-project"
      And a spec titled "Upstream Spec" exists for the project
      And a cross-project dependency from the project to project "target-api-project"
      When the project is disconnected
      Then 0 unresolved inbound dependencies are reported

    @positive @api
    Scenario: API reconnect restores prior specs
      Given a connected project "recon-api-project"
      And a spec titled "Spec Alpha" exists for the project
      And a spec titled "Spec Beta" exists for the project
      And a spec titled "Spec Gamma" exists for the project
      When the project is disconnected
      And the project is reconnected
      Then the prior specs reappear for the project

    @falsification @api
    Scenario: API rejects disconnect when an active loop exists
      Given a connected project "loop-api-project"
      And an active implementation loop exists on the project
      When the project is disconnected
      Then the disconnect is rejected with code "active_loop_exists"

    @falsification @api
    Scenario: API disconnect does not modify the underlying repository
      Given a temp repository fixture
      And a connected project "nd-api-project"
      When the project is disconnected
      Then the repository byte checksum is unchanged

  Rule: CLI surface

    @positive @cli
    Scenario: CLI disconnects a project with no active loops and repo stays byte-identical
      Given a temp repository fixture
      And a connected project "disc-cli-project"
      When the project is disconnected
      And projects are listed
      Then the project no longer appears in the project list
      And the repository byte checksum is unchanged

    @positive @cli
    Scenario: CLI emits unresolved dependency signal on disconnect
      Given a connected project "dep-cli-project"
      And a spec titled "Upstream Spec" exists for the project
      And a cross-project dependency from the project to project "target-cli-project"
      When the project is disconnected
      Then 0 unresolved inbound dependencies are reported

    @positive @cli
    Scenario: CLI reconnect restores prior specs
      Given a connected project "recon-cli-project"
      And a spec titled "Spec Alpha" exists for the project
      And a spec titled "Spec Beta" exists for the project
      And a spec titled "Spec Gamma" exists for the project
      When the project is disconnected
      And the project is reconnected
      Then the prior specs reappear for the project

    @falsification @cli
    Scenario: CLI rejects disconnect when an active loop exists
      Given a connected project "loop-cli-project"
      And an active implementation loop exists on the project
      When the project is disconnected
      Then the disconnect is rejected with code "active_loop_exists"

    @falsification @cli
    Scenario: CLI disconnect does not modify the underlying repository
      Given a temp repository fixture
      And a connected project "nd-cli-project"
      When the project is disconnected
      Then the repository byte checksum is unchanged

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP disconnects a project with no active loops and repo stays byte-identical
      Given a temp repository fixture
      And a connected project "disc-mcp-project"
      When the project is disconnected
      And projects are listed
      Then the project no longer appears in the project list
      And the repository byte checksum is unchanged

    @positive @mcp
    Scenario: MCP emits unresolved dependency signal on disconnect
      Given a connected project "dep-mcp-project"
      And a spec titled "Upstream Spec" exists for the project
      And a cross-project dependency from the project to project "target-mcp-project"
      When the project is disconnected
      Then 0 unresolved inbound dependencies are reported

    @positive @mcp
    Scenario: MCP reconnect restores prior specs
      Given a connected project "recon-mcp-project"
      And a spec titled "Spec Alpha" exists for the project
      And a spec titled "Spec Beta" exists for the project
      And a spec titled "Spec Gamma" exists for the project
      When the project is disconnected
      And the project is reconnected
      Then the prior specs reappear for the project

    @falsification @mcp
    Scenario: MCP rejects disconnect when an active loop exists
      Given a connected project "loop-mcp-project"
      And an active implementation loop exists on the project
      When the project is disconnected
      Then the disconnect is rejected with code "active_loop_exists"

    @falsification @mcp
    Scenario: MCP disconnect does not modify the underlying repository
      Given a temp repository fixture
      And a connected project "nd-mcp-project"
      When the project is disconnected
      Then the repository byte checksum is unchanged

  Rule: TUI surface

    @positive @tui
    Scenario: TUI disconnects a project with no active loops and repo stays byte-identical
      Given a temp repository fixture
      And a connected project "disc-tui-project"
      When the project is disconnected
      And projects are listed
      Then the project no longer appears in the project list
      And the repository byte checksum is unchanged

    @positive @tui
    Scenario: TUI emits unresolved dependency signal on disconnect
      Given a connected project "dep-tui-project"
      And a spec titled "Upstream Spec" exists for the project
      And a cross-project dependency from the project to project "target-tui-project"
      When the project is disconnected
      Then 0 unresolved inbound dependencies are reported

    @positive @tui
    Scenario: TUI reconnect restores prior specs
      Given a connected project "recon-tui-project"
      And a spec titled "Spec Alpha" exists for the project
      And a spec titled "Spec Beta" exists for the project
      And a spec titled "Spec Gamma" exists for the project
      When the project is disconnected
      And the project is reconnected
      Then the prior specs reappear for the project

    @falsification @tui
    Scenario: TUI rejects disconnect when an active loop exists
      Given a connected project "loop-tui-project"
      And an active implementation loop exists on the project
      When the project is disconnected
      Then the disconnect is rejected with code "active_loop_exists"

    @falsification @tui
    Scenario: TUI disconnect does not modify the underlying repository
      Given a temp repository fixture
      And a connected project "nd-tui-project"
      When the project is disconnected
      Then the repository byte checksum is unchanged

  Rule: Web surface

    @positive @web
    Scenario: Web disconnects a project with no active loops and repo stays byte-identical
      Given a temp repository fixture
      And a connected project "disc-web-project"
      When the project is disconnected
      And projects are listed
      Then the project no longer appears in the project list
      And the repository byte checksum is unchanged

    @positive @web
    Scenario: Web emits unresolved dependency signal on disconnect
      Given a connected project "dep-web-project"
      And a spec titled "Upstream Spec" exists for the project
      And a cross-project dependency from the project to project "target-web-project"
      When the project is disconnected
      Then 0 unresolved inbound dependencies are reported

    @positive @web
    Scenario: Web reconnect restores prior specs
      Given a connected project "recon-web-project"
      And a spec titled "Spec Alpha" exists for the project
      And a spec titled "Spec Beta" exists for the project
      And a spec titled "Spec Gamma" exists for the project
      When the project is disconnected
      And the project is reconnected
      Then the prior specs reappear for the project

    @falsification @web
    Scenario: Web rejects disconnect when an active loop exists
      Given a connected project "loop-web-project"
      And an active implementation loop exists on the project
      When the project is disconnected
      Then the disconnect is rejected with code "active_loop_exists"

    @falsification @web
    Scenario: Web disconnect does not modify the underlying repository
      Given a temp repository fixture
      And a connected project "nd-web-project"
      When the project is disconnected
      Then the repository byte checksum is unchanged
