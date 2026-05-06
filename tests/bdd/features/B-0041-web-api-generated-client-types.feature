@B-0041
Feature: Web API client consumes OpenAPI-generated TypeScript types
  The web frontend's account client imports organization and project
  request/response shapes from the OpenAPI-generated type module
  instead of maintaining handwritten duplicates. The generated types
  ensure the client and server wire contract stays in sync without
  manual maintenance. B-0041 is proved on the web and api interfaces.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API client lists organizations using generated types
      Given alice has signed up with email "alice-api-gen@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      When alice lists their organizations
      Then alice sees 2 organization memberships

    @positive @api
    Scenario: API client switches active organization using generated types
      Given alice has signed up with email "alice-api-sw@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      And organization "00000000-0000-0000-0000-000000000001" has project "00000000-0000-0000-0000-000000000101" named "Alpha Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"

    @positive @api
    Scenario: API client lists projects using generated types
      Given alice has signed up with email "alice-api-proj@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha"
      And organization "00000000-0000-0000-0000-000000000001" has project "00000000-0000-0000-0000-000000000101" named "Project One"
      When alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"

    @falsification @api
    Scenario: API client error parsed via shared response parser
      Given alice has signed up with email "alice-api-err@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha"
      When alice tries to switch active organization to "00000000-0000-0000-0000-000000000999"
      Then the request fails with code "organization-not-member"

  Rule: Web surface

    @positive @web
    Scenario: Web client lists organizations using generated types
      Given alice has signed up with email "alice-web-gen@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      When alice lists their organizations
      Then alice sees 2 organization memberships

    @positive @web
    Scenario: Web client switches active organization using generated types
      Given alice has signed up with email "alice-web-sw@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha" and organization "00000000-0000-0000-0000-000000000002" named "Beta"
      And organization "00000000-0000-0000-0000-000000000001" has project "00000000-0000-0000-0000-000000000101" named "Alpha Project"
      When alice switches active organization to "00000000-0000-0000-0000-000000000001"
      And alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"

    @positive @web
    Scenario: Web client lists projects using generated types
      Given alice has signed up with email "alice-web-proj@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha"
      And organization "00000000-0000-0000-0000-000000000001" has project "00000000-0000-0000-0000-000000000101" named "Project One"
      When alice lists the active organization projects
      Then alice sees only projects belonging to "00000000-0000-0000-0000-000000000001"

    @falsification @web
    Scenario: Web client error parsed via shared response parser
      Given alice has signed up with email "alice-web-err@example.com" and password "p4ssw0rd"
      And alice has signed up and belongs to organization "00000000-0000-0000-0000-000000000001" named "Alpha"
      When alice tries to switch active organization to "00000000-0000-0000-0000-000000000999"
      Then the request fails with code "organization-not-member"
