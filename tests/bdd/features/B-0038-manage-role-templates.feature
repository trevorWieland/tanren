# expected-evidence-id: B-0038-bdd-positive-falsification
# evidence-id: B-0038-bdd-positive-falsification
# witness-command: cargo run -q -p tanren-bdd --bin tanren-bdd-runner --locked -- --tags @B-0038
# expected-evidence-id: web-b0038-missing
# evidence-id: web-b0038-missing
# witness-command: pnpm --filter @tanren/web exec playwright test --grep B-0038
@B-0038
Feature: Manage roles as permission templates
  A team-builder can create, edit, and delete role templates that bundle
  permissions. Applying a role grants direct permissions at that moment;
  editing or deleting a role does not retroactively change existing grants.
  Authorization checks resolve on permissions only and reject role principals.
  Each interface (`web`, `api`, `mcp`, `cli`, `tui`) repeats positive lifecycle
  witnesses plus falsification witnesses for role-principal rejection and input

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
      When the operator applies the role template to account principal alice
      Then applying the role template to account principal alice is idempotent for permissions "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer API Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"
      When the operator checks permission "project.read" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.read"
      When the operator checks permission "project.comment" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.comment"
      When the operator checks permission "project.read" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.read"
      When the operator checks permission "project.merge" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.merge"

    @falsification @api
    Scenario: API permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor API" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.audit"
      When the operator checks permission "project.merge" for account principal alice
      Then the permission check result is denied
      And the permission check has no matching grant ids
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

    @falsification @api
    Scenario: API role validation rejects empty or oversized bundles, incompatible grant scopes, and missing principals
      When the operator attempts to create role template "Empty API" with 0 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator attempts to create role template "Oversized API" with 65 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator creates role template "Scope Guard API" with permissions "project.read"
      When the operator attempts to apply the role template with an account grant-scope mismatch
      Then the role request fails with code "validation_failed"
      When the operator attempts to apply the role template to missing account principal ghost
      Then the role request fails with code "not_found"
      When the operator checks permission "project.read" for missing account principal ghost
      Then the role request fails with code "not_found"


    @falsification @api
    Scenario: API concurrent role application produces exactly one stable grant per permission
      When the operator creates role template "Race API" with permissions "project.read,project.write"
      Then the active role template has permissions "project.read,project.write"
      When 10 concurrent applications apply the role template to account principal alice
      Then exactly one direct grant per permission exists for account principal alice with permissions "project.read,project.write"
      And the direct grant ids for account principal alice match across 2 repeated observations of permissions "project.read,project.write"

  Rule: MCP surface

    @positive @mcp
    Scenario: MCP role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer MCP" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then applying the role template to account principal alice is idempotent for permissions "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer MCP Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"
      When the operator checks permission "project.read" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.read"
      When the operator checks permission "project.comment" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.comment"
      When the operator checks permission "project.read" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.read"
      When the operator checks permission "project.merge" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.merge"

    @falsification @mcp
    Scenario: MCP permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor MCP" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.audit"
      When the operator checks permission "project.merge" for account principal alice
      Then the permission check result is denied
      And the permission check has no matching grant ids
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

    @falsification @mcp
    Scenario: MCP role validation rejects empty or oversized bundles, incompatible grant scopes, and missing principals
      When the operator attempts to create role template "Empty MCP" with 0 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator attempts to create role template "Oversized MCP" with 65 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator creates role template "Scope Guard MCP" with permissions "project.read"
      When the operator attempts to apply the role template with an account grant-scope mismatch
      Then the role request fails with code "validation_failed"
      When the operator attempts to apply the role template to missing account principal ghost
      Then the role request fails with code "not_found"
      When the operator checks permission "project.read" for missing account principal ghost
      Then the role request fails with code "not_found"


    @falsification @mcp
    Scenario: MCP concurrent role application produces exactly one stable grant per permission
      When the operator creates role template "Race MCP" with permissions "project.read,project.write"
      Then the active role template has permissions "project.read,project.write"
      When 10 concurrent applications apply the role template to account principal alice
      Then exactly one direct grant per permission exists for account principal alice with permissions "project.read,project.write"
      And the direct grant ids for account principal alice match across 2 repeated observations of permissions "project.read,project.write"

  Rule: CLI surface

    @positive @cli
    Scenario: CLI role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer CLI" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then applying the role template to account principal alice is idempotent for permissions "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer CLI Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"
      When the operator checks permission "project.read" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.read"
      When the operator checks permission "project.comment" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.comment"
      When the operator checks permission "project.read" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.read"
      When the operator checks permission "project.merge" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.merge"

    @falsification @cli
    Scenario: CLI permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor CLI" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.audit"
      When the operator checks permission "project.merge" for account principal alice
      Then the permission check result is denied
      And the permission check has no matching grant ids
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

    @falsification @cli
    Scenario: CLI role validation rejects empty or oversized bundles, incompatible grant scopes, and missing principals
      When the operator attempts to create role template "Empty CLI" with 0 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator attempts to create role template "Oversized CLI" with 65 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator creates role template "Scope Guard CLI" with permissions "project.read"
      When the operator attempts to apply the role template with an account grant-scope mismatch
      Then the role request fails with code "validation_failed"
      When the operator attempts to apply the role template to missing account principal ghost
      Then the role request fails with code "not_found"
      When the operator checks permission "project.read" for missing account principal ghost
      Then the role request fails with code "not_found"


    @falsification @cli
    Scenario: CLI concurrent role application produces exactly one stable grant per permission
      When the operator creates role template "Race CLI" with permissions "project.read,project.write"
      Then the active role template has permissions "project.read,project.write"
      When 10 concurrent applications apply the role template to account principal alice
      Then exactly one direct grant per permission exists for account principal alice with permissions "project.read,project.write"
      And the direct grant ids for account principal alice match across 2 repeated observations of permissions "project.read,project.write"

  Rule: TUI surface

    @positive @tui
    Scenario: TUI role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer TUI" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then applying the role template to account principal alice is idempotent for permissions "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer TUI Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"
      When the operator checks permission "project.read" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.read"
      When the operator checks permission "project.comment" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.comment"
      When the operator checks permission "project.read" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.read"
      When the operator checks permission "project.merge" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.merge"

    @falsification @tui
    Scenario: TUI permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor TUI" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.audit"
      When the operator checks permission "project.merge" for account principal alice
      Then the permission check result is denied
      And the permission check has no matching grant ids
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

    @falsification @tui
    Scenario: TUI role validation rejects empty or oversized bundles, incompatible grant scopes, and missing principals
      When the operator attempts to create role template "Empty TUI" with 0 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator attempts to create role template "Oversized TUI" with 65 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator creates role template "Scope Guard TUI" with permissions "project.read"
      When the operator attempts to apply the role template with an account grant-scope mismatch
      Then the role request fails with code "validation_failed"
      When the operator attempts to apply the role template to missing account principal ghost
      Then the role request fails with code "not_found"
      When the operator checks permission "project.read" for missing account principal ghost
      Then the role request fails with code "not_found"

    @falsification @tui
    Scenario: TUI concurrent role application produces exactly one stable grant per permission
      When the operator creates role template "Race TUI" with permissions "project.read,project.write"
      Then the active role template has permissions "project.read,project.write"
      When 10 concurrent applications apply the role template to account principal alice
      Then exactly one direct grant per permission exists for account principal alice with permissions "project.read,project.write"
      And the direct grant ids for account principal alice match across 2 repeated observations of permissions "project.read,project.write"

  Rule: Web surface

    @positive @web
    Scenario: Web role templates apply grants non-retroactively and survive deletion
      When the operator creates role template "Reviewer Web" with permissions "project.read,project.comment"
      Then the active role template has permissions "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then account principal alice has direct grants "project.read,project.comment"
      When the operator applies the role template to account principal alice
      Then applying the role template to account principal alice is idempotent for permissions "project.read,project.comment"
      When the operator edits the active role template to name "Reviewer Web Plus" and permissions "project.read,project.comment,project.merge"
      Then the active role template has permissions "project.read,project.comment,project.merge"
      And account principal alice retains direct grants "project.read,project.comment"
      When the operator applies the role template to account principal bob
      Then account principal bob has direct grants "project.read,project.comment,project.merge"
      When the operator deletes the active role template
      Then the active role template no longer exists
      And account principal alice retains direct grants "project.read,project.comment"
      And account principal bob retains direct grants "project.read,project.comment,project.merge"
      When the operator checks permission "project.read" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.read"
      When the operator checks permission "project.comment" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.comment"
      When the operator checks permission "project.read" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.read"
      When the operator checks permission "project.merge" for account principal bob
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal bob and permission "project.merge"

    @falsification @web
    Scenario: Web permission checks accept account principals and reject role principals
      When the operator creates role template "Auditor Web" with permissions "project.audit"
      And the operator applies the role template to account principal alice
      When the operator checks permission "project.audit" for account principal alice
      Then the permission check result is allowed
      And the permission check matches direct grant ids for account principal alice and permission "project.audit"
      When the operator checks permission "project.merge" for account principal alice
      Then the permission check result is denied
      And the permission check has no matching grant ids
      When the operator checks permission "project.audit" for the role template principal
      Then the role request fails with code "role_as_principal_rejected"

    @falsification @web
    Scenario: Web role validation rejects empty or oversized bundles, incompatible grant scopes, and missing principals
      When the operator attempts to create role template "Empty Web" with 0 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator attempts to create role template "Oversized Web" with 65 synthetic permissions
      Then the role request fails with code "validation_failed"
      When the operator creates role template "Scope Guard Web" with permissions "project.read"
      When the operator attempts to apply the role template with an account grant-scope mismatch
      Then the role request fails with code "validation_failed"
      When the operator attempts to apply the role template to missing account principal ghost
      Then the role request fails with code "not_found"
      When the operator checks permission "project.read" for missing account principal ghost
      Then the role request fails with code "not_found"

    @falsification @web
    Scenario: Web concurrent role application produces exactly one stable grant per permission
      When the operator creates role template "Race Web" with permissions "project.read,project.write"
      Then the active role template has permissions "project.read,project.write"
      When 10 concurrent applications apply the role template to account principal alice
      Then exactly one direct grant per permission exists for account principal alice with permissions "project.read,project.write"
      And the direct grant ids for account principal alice match across 2 repeated observations of permissions "project.read,project.write"
