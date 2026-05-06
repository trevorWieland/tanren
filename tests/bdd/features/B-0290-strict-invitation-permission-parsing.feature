@B-0290
Feature: Reject corrupted invitation permissions on acceptance
  When an invitation row contains an org_permissions value that fails
  domain validation, the store must propagate a data-invariant error
  rather than silently dropping the value to None. Valid permissions
  must continue to populate membership correctly.

  Background:
    Given a clean Tanren environment

  Rule: CLI surface

    @positive @cli
    Scenario: Valid org_permissions populate membership on existing-account join
      Given alice has signed up with email "alice-valid-perms-cli@example.com" and password "p4ssw0rd"
      And a pending invitation for "alice-valid-perms-cli@example.com" with token "valid-perm-cli-token-padpad" and "admin" permissions
      When alice joins organization with invitation "valid-perm-cli-token-padpad"
      Then alice is a member of the inviting organization
      And alice has been granted "admin" organization permissions

    @falsification @cli
    Scenario: Corrupted org_permissions reject existing-account join
      Given alice has signed up with email "alice-corrupt-cli@example.com" and password "p4ssw0rd"
      And a corrupted invitation for "alice-corrupt-cli@example.com" with token "corrupt-cli-token-padpad"
      When alice joins organization with invitation "corrupt-cli-token-padpad"
      Then the request fails with code "validation_failed"
