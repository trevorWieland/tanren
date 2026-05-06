@B-0125
Feature: Store user credentials without exposing secret values
  A user can add, update, and remove their own credentials through every
  Tanren interface. After submission, the stored secret value is never
  returned or projected in any response, view, log, or audit entry.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API user adds a credential and verifies kind, scope, and last-updated metadata
      When alice self-signs up with email "alice-api-meta@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "api-meta-key" with secret "sk-api-Ax7mK3pR9vNqW2jF"
      Then the response contains kind "api_key" and scope and last-updated timestamp
      And the response does not contain the value "sk-api-Ax7mK3pR9vNqW2jF"

    @positive @api
    Scenario: API user updates a credential and sees only redacted output
      Given alice has signed up with email "alice-api-redact@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "api-redact-key" with secret "sk-api-Bx8mL4qS0wOrX3kG"
      When alice updates credential "api-redact-key" with new secret "sk-api-Cx9mM5rT1yPsY4lH"
      Then the response contains kind and scope but no secret value
      And the API output is redacted and does not contain "sk-api-Cx9mM5rT1yPsY4lH"

    @falsification @api
    Scenario: API read path for stored credential value returns only redacted or absent value
      Given alice has signed up with email "alice-api-read@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "api-read-key" with secret "sk-api-Dx0mN6rU2zQtZ5mI"
      When alice queries every credential read path for "api-read-key"
      Then every response returns redacted or absent value and never "sk-api-Dx0mN6rU2zQtZ5mI"

    @falsification @api
    Scenario: API event and audit history contain zero occurrences of the submitted raw credential value
      Given alice has signed up with email "alice-api-audit@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "api-audit-key" with secret "sk-api-Ex1mO7sV3aRuW6nJ"
      Then recent event history contains zero occurrences of "sk-api-Ex1mO7sV3aRuW6nJ"
      And captured API output contains zero occurrences of "sk-api-Ex1mO7sV3aRuW6nJ"

    @falsification @api
    Scenario: API sign-up password is never returned or recorded in event or audit output
      When alice self-signs up with email "alice-api-pw@example.com" and password "Px2mF8kT4wQvY7nBzR"
      Then the response does not contain the value "Px2mF8kT4wQvY7nBzR"
      And recent event history contains zero occurrences of "Px2mF8kT4wQvY7nBzR"
      And captured API output contains zero occurrences of "Px2mF8kT4wQvY7nBzR"

  Rule: Web surface

    @positive @web
    Scenario: Web user adds a credential and verifies kind, scope, and last-updated metadata
      When alice self-signs up with email "alice-web-meta@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "web-meta-key" with secret "sk-web-Fx3mG9lU5bSvX8oK"
      Then the response contains kind "api_key" and scope and last-updated timestamp
      And the response does not contain the value "sk-web-Fx3mG9lU5bSvX8oK"

    @positive @web
    Scenario: Web user updates a credential and page content displays only redacted output
      Given alice has signed up with email "alice-web-redact@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "web-redact-key" with secret "sk-web-Gx4mH0mV6cTwY9pL"
      When alice updates credential "web-redact-key" with new secret "sk-web-Hx5mI1nW7dUxZ0qM"
      Then the page content shows only redacted credential output
      And the page content does not contain "sk-web-Hx5mI1nW7dUxZ0qM"

    @falsification @web
    Scenario: Web read path for stored credential value returns only redacted or absent value
      Given alice has signed up with email "alice-web-read@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "web-read-key" with secret "sk-web-Ix6mJ2oX8eVyA1rN"
      When alice queries every credential read path for "web-read-key"
      Then every response returns redacted or absent value and never "sk-web-Ix6mJ2oX8eVyA1rN"

    @falsification @web
    Scenario: Web event and audit history contain zero occurrences of the submitted raw credential value
      Given alice has signed up with email "alice-web-audit@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "web-audit-key" with secret "sk-web-Jx7mK3pY9fWzB2sO"
      Then recent event history contains zero occurrences of "sk-web-Jx7mK3pY9fWzB2sO"
      And captured web output contains zero occurrences of "sk-web-Jx7mK3pY9fWzB2sO"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI user adds a credential and verifies kind, scope, and last-updated metadata
      When alice self-signs up with email "alice-cli-meta@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "cli-meta-key" with secret "sk-cli-Kx8mL4qZ0gXaC3tP"
      Then the response contains kind "api_key" and scope and last-updated timestamp
      And the response does not contain the value "sk-cli-Kx8mL4qZ0gXaC3tP"

    @positive @cli
    Scenario: CLI user updates a credential and terminal output is redacted
      Given alice has signed up with email "alice-cli-redact@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "cli-redact-key" with secret "sk-cli-Lx9mM5rA1hYbD4uQ"
      When alice updates credential "cli-redact-key" with new secret "sk-cli-Mx0mN6sB2iZcE5vR"
      Then the terminal output shows only redacted credential output
      And the terminal output does not contain "sk-cli-Mx0mN6sB2iZcE5vR"

    @falsification @cli
    Scenario: CLI read path for stored credential value returns only redacted or absent value
      Given alice has signed up with email "alice-cli-read@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "cli-read-key" with secret "sk-cli-Nx1mO7tC3jAdF6wS"
      When alice queries every credential read path for "cli-read-key"
      Then every response returns redacted or absent value and never "sk-cli-Nx1mO7tC3jAdF6wS"

    @falsification @cli
    Scenario: CLI event and audit history contain zero occurrences of the submitted raw credential value
      Given alice has signed up with email "alice-cli-audit@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "cli-audit-key" with secret "sk-cli-Ox2mP8uD4kBeG7xT"
      Then recent event history contains zero occurrences of "sk-cli-Ox2mP8uD4kBeG7xT"
      And captured CLI output contains zero occurrences of "sk-cli-Ox2mP8uD4kBeG7xT"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP user adds a credential and verifies kind, scope, and last-updated metadata
      When alice self-signs up with email "alice-mcp-meta@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "mcp-meta-key" with secret "sk-mcp-Px3mQ9vE5lCfH8yU"
      Then the response contains kind "api_key" and scope and last-updated timestamp
      And the response does not contain the value "sk-mcp-Px3mQ9vE5lCfH8yU"

    @positive @mcp
    Scenario: MCP user updates a credential and tool return is redacted
      Given alice has signed up with email "alice-mcp-redact@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "mcp-redact-key" with secret "sk-mcp-Qx4mR0wF6mDgI9zV"
      When alice updates credential "mcp-redact-key" with new secret "sk-mcp-Rx5mS1xG7nEhJ0aW"
      Then the tool return shows only redacted credential output
      And the tool return does not contain "sk-mcp-Rx5mS1xG7nEhJ0aW"

    @falsification @mcp
    Scenario: MCP read path for stored credential value returns only redacted or absent value
      Given alice has signed up with email "alice-mcp-read@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "mcp-read-key" with secret "sk-mcp-Sx6mT2yH8oFiK1bX"
      When alice queries every credential read path for "mcp-read-key"
      Then every response returns redacted or absent value and never "sk-mcp-Sx6mT2yH8oFiK1bX"

    @falsification @mcp
    Scenario: MCP event and audit history contain zero occurrences of the submitted raw credential value
      Given alice has signed up with email "alice-mcp-audit@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "mcp-audit-key" with secret "sk-mcp-Tx7mU3zI9pGjL2cY"
      Then recent event history contains zero occurrences of "sk-mcp-Tx7mU3zI9pGjL2cY"
      And captured MCP output contains zero occurrences of "sk-mcp-Tx7mU3zI9pGjL2cY"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI user adds a credential and verifies kind, scope, and last-updated metadata
      When alice self-signs up with email "alice-tui-meta@example.com" and password "p4ssw0rd"
      Then alice receives a session token
      When alice adds an api_key credential named "tui-meta-key" with secret "sk-tui-Ux8mV4aJ0qHkM3dZ"
      Then the response contains kind "api_key" and scope and last-updated timestamp
      And the response does not contain the value "sk-tui-Ux8mV4aJ0qHkM3dZ"

    @positive @tui
    Scenario: TUI user updates a credential and screen content displays only redacted output
      Given alice has signed up with email "alice-tui-redact@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "tui-redact-key" with secret "sk-tui-Vx9mW5bK1rIlN4eA"
      When alice updates credential "tui-redact-key" with new secret "sk-tui-Wx0mX6cL2sJmO5fB"
      Then the screen content shows only redacted credential output
      And the screen content does not contain "sk-tui-Wx0mX6cL2sJmO5fB"

    @falsification @tui
    Scenario: TUI read path for stored credential value returns only redacted or absent value
      Given alice has signed up with email "alice-tui-read@example.com" and password "p4ssw0rd"
      And alice has added an api_key credential named "tui-read-key" with secret "sk-tui-Xx1mY7dM3tKnP6gC"
      When alice queries every credential read path for "tui-read-key"
      Then every response returns redacted or absent value and never "sk-tui-Xx1mY7dM3tKnP6gC"

    @falsification @tui
    Scenario: TUI event and audit history contain zero occurrences of the submitted raw credential value
      Given alice has signed up with email "alice-tui-audit@example.com" and password "p4ssw0rd"
      When alice adds an api_key credential named "tui-audit-key" with secret "sk-tui-Yx2mZ8eN4uLoQ7hD"
      Then recent event history contains zero occurrences of "sk-tui-Yx2mZ8eN4uLoQ7hD"
      And captured TUI output contains zero occurrences of "sk-tui-Yx2mZ8eN4uLoQ7hD"
