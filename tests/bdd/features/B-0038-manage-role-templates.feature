# expected-evidence-id: B-0038-bdd-positive-falsification
# evidence-id: B-0038-bdd-positive-falsification
# witness-command: cargo run -q -p tanren-bdd --bin tanren-bdd-runner --locked -- --tags @B-0038
@B-0038
Feature: Manage roles as permission templates
  A team-builder can create, edit, and delete role templates that bundle
  permissions. Applying a role grants direct permissions at that moment;
  editing or deleting a role does not retroactively change existing grants.
  Authorization checks resolve on permissions only and reject role principals.
  Each interface (`web`, `api`, `mcp`, `cli`, `tui`) repeats the same two
  witness shapes: role lifecycle + non-retroactivity as a positive witness,
  and role-as-principal permission-check rejection as a falsification witness.

  Background:
    Given a clean role-template environment
    And an organization role scope

  Rule: API surface

    @positive @api
    Scenario: API role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer API" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer API Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"

    @falsification @api
    Scenario: API permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor API" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

  Rule: Web surface

    @positive @web
    Scenario: Web role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer Web" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer Web Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"

    @falsification @web
    Scenario: Web permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor Web" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer MCP" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer MCP Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"

    @falsification @mcp
    Scenario: MCP permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor MCP" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer CLI" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer CLI Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"

    @falsification @cli
    Scenario: CLI permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor CLI" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer TUI" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer TUI Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"

    @falsification @tui
    Scenario: TUI permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor TUI" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"
