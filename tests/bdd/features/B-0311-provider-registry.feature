@B-0311
Feature: Provider registry wiring for project commands
  Production API and MCP runtimes resolve source-control providers
  through a registry abstraction. When no provider is configured,
  project commands fail with provider_not_configured before any
  external side effects. When a provider is available through the
  registry, project commands succeed normally.

  Background:
    Given a signed-in account "alice"

  Rule: API surface

    @positive @api
    Scenario: Connect an existing repo over the API succeeds when the provider registry has a configured provider
      Given an existing repository "registry-api-repo" that alice can access
      When alice connects the repository "registry-api-repo" to her account
      Then the project "registry-api-repo" appears in alice's account

    @positive @api
    Scenario: Create a new project over the API succeeds when the provider registry has a configured provider
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "registry-api-new" at host "git.example.com"
      Then the project "registry-api-new" appears in alice's account

    @falsification @api
    Scenario: Connect an existing repo over the API fails with provider_not_configured when no provider is registered
      Given the provider is not configured
      When alice connects the repository "registry-api-no-provider" to her account
      Then the request fails with code "provider_not_configured"

    @falsification @api
    Scenario: Create a new project over the API fails with provider_not_configured when no provider is registered
      Given the provider is not configured
      When alice creates a new project named "registry-api-no-provider-new" at host "git.example.com"
      Then the request fails with code "provider_not_configured"

  Rule: MCP surface

    @positive @mcp
    Scenario: Connect an existing repo over MCP succeeds when the provider registry has a configured provider
      Given an existing repository "registry-mcp-repo" that alice can access
      When alice connects the repository "registry-mcp-repo" to her account
      Then the project "registry-mcp-repo" appears in alice's account

    @positive @mcp
    Scenario: Create a new project over MCP succeeds when the provider registry has a configured provider
      Given a fixture SCM host "git.example.com" that alice can access
      When alice creates a new project named "registry-mcp-new" at host "git.example.com"
      Then the project "registry-mcp-new" appears in alice's account

    @falsification @mcp
    Scenario: Connect an existing repo over MCP fails with provider_not_configured when no provider is registered
      Given the provider is not configured
      When alice connects the repository "registry-mcp-no-provider" to her account
      Then the request fails with code "provider_not_configured"

    @falsification @mcp
    Scenario: Create a new project over MCP fails with provider_not_configured when no provider is registered
      Given the provider is not configured
      When alice creates a new project named "registry-mcp-no-provider-new" at host "git.example.com"
      Then the request fails with code "provider_not_configured"
