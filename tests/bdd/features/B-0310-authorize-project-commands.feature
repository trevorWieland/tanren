@B-0310
Feature: Authorize project commands from authenticated actor context
  Project registration commands evaluate a typed authorization decision after
  the caller is authenticated and before any provider or store side effects.
  The actor context is derived from authenticated credentials, not from
  caller-supplied parameters. Personal-scope registration is allowed for any
  authenticated account. Organization-scope registration is allowed only when
  the actor's organization matches the requested org.

  Background:
    Given a signed-in account "alice"

  Rule: API surface

    @positive @api
    Scenario: Personal-scope project connect over the API is allowed for an authenticated account
      Given an existing repository "api-policy-personal-repo" that alice can access
      When alice connects the repository "api-policy-personal-repo" to her account
      Then the project "api-policy-personal-repo" appears in alice's account

    @positive @api
    Scenario: Personal-scope project create over the API is allowed for an authenticated account
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "api-policy-personal-new" at host "git.example.com"
      Then the project "api-policy-personal-new" appears in alice's account

    @falsification @api
    Scenario: Org-scoped project connect over the API is denied when the actor is not a member of the target org
      Given an existing repository "api-policy-org-denied-repo" that alice can access
      When alice connects the repository "api-policy-org-denied-repo" to an org she is not a member of
      Then the request fails with code "access_denied"

    @falsification @api
    Scenario: Org-scoped project create over the API is denied when the actor is not a member of the target org
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "api-policy-org-denied-new" at host "git.example.com" under an org she is not a member of
      Then the request fails with code "access_denied"

  Rule: MCP surface

    @positive @mcp
    Scenario: Personal-scope project connect over MCP is allowed for an authenticated capability context
      Given an existing repository "mcp-policy-personal-repo" that alice can access
      When alice connects the repository "mcp-policy-personal-repo" to her account
      Then the project "mcp-policy-personal-repo" appears in alice's account

    @positive @mcp
    Scenario: Personal-scope project create over MCP is allowed for an authenticated capability context
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "mcp-policy-personal-new" at host "git.example.com"
      Then the project "mcp-policy-personal-new" appears in alice's account

    @falsification @mcp
    Scenario: Org-scoped project connect over MCP is denied when the actor is not a member of the target org
      Given an existing repository "mcp-policy-org-denied-repo" that alice can access
      When alice connects the repository "mcp-policy-org-denied-repo" to an org she is not a member of
      Then the request fails with code "access_denied"

    @falsification @mcp
    Scenario: Org-scoped project create over MCP is denied when the actor is not a member of the target org
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "mcp-policy-org-denied-new" at host "git.example.com" under an org she is not a member of
      Then the request fails with code "access_denied"
