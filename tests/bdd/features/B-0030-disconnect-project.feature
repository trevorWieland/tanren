@B-0030
Feature: Disconnect a project from Tanren
  A solo-builder or team-builder can disconnect a project from their
  Tanren account so that it no longer appears in their views, without
  affecting the underlying repository. Each interface in B-0030 repeats
  the same witnesses required by the spec's expected_evidence: positive
  disconnect, rejected disconnect due to active loop, and unresolved
  cross-project dependency signal. The interface tag is a witness label
  — every surface routes through the same Handlers facade per the
  equivalent-operations rule in interfaces.md.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API connects a project
      Given alice has signed up with email "alice-b0030-api@example.com" and password "p4ssw0rd"
      When alice connects a project named "my-api-project" with repository "https://github.com/example/api-repo"
      Then the project appears in alice's project list
      And a "project_connected" event is recorded

    @positive @api
    Scenario: API disconnects a project with no active loops
      Given alice has signed up with email "alice-b0030-disc-api@example.com" and password "p4ssw0rd"
      And alice has a connected project "disc-api-project" with repository "https://github.com/example/disc-api-repo"
      When alice disconnects project "disc-api-project"
      Then the project no longer appears in alice's project list
      And a "project_disconnected" event is recorded

    @falsification @api
    Scenario: API rejects disconnect when an active loop exists
      Given alice has signed up with email "alice-b0030-loop-api@example.com" and password "p4ssw0rd"
      And alice has a connected project "loop-api-project" with an active implementation loop
      When alice disconnects project "loop-api-project"
      Then the request fails with code "active_loop_exists"
      And a "project_disconnect_rejected" event is recorded

    @positive @api
    Scenario: API emits unresolved dependency signal on disconnect
      Given alice has signed up with email "alice-b0030-dep-api@example.com" and password "p4ssw0rd"
      And alice has a connected project "dep-api-project" with a dependency on a disconnected target
      When alice disconnects project "dep-api-project"
      And a "cross_project_dependency_unresolved" event is recorded

  Rule: CLI surface

    @positive @cli
    Scenario: CLI connects a project
      Given alice has signed up with email "alice-b0030-cli@example.com" and password "p4ssw0rd"
      When alice connects a project named "my-cli-project" with repository "https://github.com/example/cli-repo"
      Then the project appears in alice's project list
      And a "project_connected" event is recorded

    @positive @cli
    Scenario: CLI disconnects a project with no active loops
      Given alice has signed up with email "alice-b0030-disc-cli@example.com" and password "p4ssw0rd"
      And alice has a connected project "disc-cli-project" with repository "https://github.com/example/disc-cli-repo"
      When alice disconnects project "disc-cli-project"
      Then the project no longer appears in alice's project list
      And a "project_disconnected" event is recorded

    @falsification @cli
    Scenario: CLI rejects disconnect when an active loop exists
      Given alice has signed up with email "alice-b0030-loop-cli@example.com" and password "p4ssw0rd"
      And alice has a connected project "loop-cli-project" with an active implementation loop
      When alice disconnects project "loop-cli-project"
      Then the request fails with code "active_loop_exists"
      And a "project_disconnect_rejected" event is recorded

    @positive @cli
    Scenario: CLI emits unresolved dependency signal on disconnect
      Given alice has signed up with email "alice-b0030-dep-cli@example.com" and password "p4ssw0rd"
      And alice has a connected project "dep-cli-project" with a dependency on a disconnected target
      When alice disconnects project "dep-cli-project"
      And a "cross_project_dependency_unresolved" event is recorded

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP connects a project
      Given alice has signed up with email "alice-b0030-mcp@example.com" and password "p4ssw0rd"
      When alice connects a project named "my-mcp-project" with repository "https://github.com/example/mcp-repo"
      Then the project appears in alice's project list
      And a "project_connected" event is recorded

    @positive @mcp
    Scenario: MCP disconnects a project with no active loops
      Given alice has signed up with email "alice-b0030-disc-mcp@example.com" and password "p4ssw0rd"
      And alice has a connected project "disc-mcp-project" with repository "https://github.com/example/disc-mcp-repo"
      When alice disconnects project "disc-mcp-project"
      Then the project no longer appears in alice's project list
      And a "project_disconnected" event is recorded

    @falsification @mcp
    Scenario: MCP rejects disconnect when an active loop exists
      Given alice has signed up with email "alice-b0030-loop-mcp@example.com" and password "p4ssw0rd"
      And alice has a connected project "loop-mcp-project" with an active implementation loop
      When alice disconnects project "loop-mcp-project"
      Then the request fails with code "active_loop_exists"
      And a "project_disconnect_rejected" event is recorded

    @positive @mcp
    Scenario: MCP emits unresolved dependency signal on disconnect
      Given alice has signed up with email "alice-b0030-dep-mcp@example.com" and password "p4ssw0rd"
      And alice has a connected project "dep-mcp-project" with a dependency on a disconnected target
      When alice disconnects project "dep-mcp-project"
      And a "cross_project_dependency_unresolved" event is recorded

  Rule: TUI surface

    @positive @tui
    Scenario: TUI connects a project
      Given alice has signed up with email "alice-b0030-tui@example.com" and password "p4ssw0rd"
      When alice connects a project named "my-tui-project" with repository "https://github.com/example/tui-repo"
      Then the project appears in alice's project list
      And a "project_connected" event is recorded

    @positive @tui
    Scenario: TUI disconnects a project with no active loops
      Given alice has signed up with email "alice-b0030-disc-tui@example.com" and password "p4ssw0rd"
      And alice has a connected project "disc-tui-project" with repository "https://github.com/example/disc-tui-repo"
      When alice disconnects project "disc-tui-project"
      Then the project no longer appears in alice's project list
      And a "project_disconnected" event is recorded

    @falsification @tui
    Scenario: TUI rejects disconnect when an active loop exists
      Given alice has signed up with email "alice-b0030-loop-tui@example.com" and password "p4ssw0rd"
      And alice has a connected project "loop-tui-project" with an active implementation loop
      When alice disconnects project "loop-tui-project"
      Then the request fails with code "active_loop_exists"
      And a "project_disconnect_rejected" event is recorded

    @positive @tui
    Scenario: TUI emits unresolved dependency signal on disconnect
      Given alice has signed up with email "alice-b0030-dep-tui@example.com" and password "p4ssw0rd"
      And alice has a connected project "dep-tui-project" with a dependency on a disconnected target
      When alice disconnects project "dep-tui-project"
      And a "cross_project_dependency_unresolved" event is recorded

  Rule: Web surface

    @positive @web
    Scenario: Web connects a project
      Given alice has signed up with email "alice-b0030-web@example.com" and password "p4ssw0rd"
      When alice connects a project named "my-web-project" with repository "https://github.com/example/web-repo"
      Then the project appears in alice's project list
      And a "project_connected" event is recorded

    @positive @web
    Scenario: Web disconnects a project with no active loops
      Given alice has signed up with email "alice-b0030-disc-web@example.com" and password "p4ssw0rd"
      And alice has a connected project "disc-web-project" with repository "https://github.com/example/disc-web-repo"
      When alice disconnects project "disc-web-project"
      Then the project no longer appears in alice's project list
      And a "project_disconnected" event is recorded

    @falsification @web
    Scenario: Web rejects disconnect when an active loop exists
      Given alice has signed up with email "alice-b0030-loop-web@example.com" and password "p4ssw0rd"
      And alice has a connected project "loop-web-project" with an active implementation loop
      When alice disconnects project "loop-web-project"
      Then the request fails with code "active_loop_exists"
      And a "project_disconnect_rejected" event is recorded

    @positive @web
    Scenario: Web emits unresolved dependency signal on disconnect
      Given alice has signed up with email "alice-b0030-dep-web@example.com" and password "p4ssw0rd"
      And alice has a connected project "dep-web-project" with a dependency on a disconnected target
      When alice disconnects project "dep-web-project"
      And a "cross_project_dependency_unresolved" event is recorded
