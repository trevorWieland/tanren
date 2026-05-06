@B-0289
Feature: Session error taxonomy and observability
  Session-related failures are projected through the shared
  AccountFailureReason taxonomy so every interface returns the same
  structured error codes. Only the API interface is in scope for B-0289.

  Background:
    Given a clean Tanren environment

  Rule: API surface

    @positive @api
    Scenario: API returns unauthenticated code for protected org-list endpoint
      When an unauthenticated request lists organizations
      Then the error code is "unauthenticated"

    @positive @api
    Scenario: API returns unauthenticated code for protected org-switch endpoint
      When an unauthenticated request switches active organization to "00000000-0000-0000-0000-000000000001"
      Then the error code is "unauthenticated"

    @falsification @api
    Scenario: API unauthenticated request does not expose internal error details
      When an unauthenticated request lists organizations
      Then the request fails with code "unauthenticated"
