@B-0289
Feature: MCP join tool redacts session token from debug output
  The MCP account.join_organization tool accepts a bearer session token.
  The struct that carries the token must redact it from Debug output so
  that tracing, logging, and developer inspection never expose the raw
  credential. The token is still correctly converted into a SessionToken
  for store lookup and the join operation proceeds normally.

  Background:
    Given a clean Tanren environment

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP join tool processes session token to authenticate and join
      Given alice has signed up with email "alice-redact-mcp@example.com" and password "p4ssw0rd"
      And a pending invitation for "alice-redact-mcp@example.com" with token "redact-mcp-token-padpad"
      When alice joins organization with invitation "redact-mcp-token-padpad"
      Then alice is a member of the inviting organization
      And a "organization_joined" event is recorded

    @falsification @mcp
    Scenario: MCP join tool rejects an expired session token
      Given alice has signed up with email "alice-expmcp@example.com" and password "p4ssw0rd"
      And alice's session has expired
      And a pending invitation for "alice-expmcp@example.com" with token "expmcp-token-padpad"
      When alice joins organization with invitation "expmcp-token-padpad"
      Then the request fails with code "unauthenticated"
