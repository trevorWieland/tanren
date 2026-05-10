@B-0134
Feature: Upgrade installed Tanren assets
  A builder can preview and confirm repository asset upgrades while preserving
  user-owned files and surfacing migration concerns before apply.

  Rule: Upgrade preview, confirmation, and apply behavior

    # rationale: CLI is the command under proof while web-tag evidence shares the same behavior text.
    @positive @cli @web
    Scenario: Preview reports migration concern and performs no writes before confirmation
      Given a clean repository fixture
      And an installed repository fixture snapshot "v1" with profile "rust-cargo" and integrations "codex"
      And legacy standards path "profiles/rust-cargo/legacy/renamed-standard.md" is marked as a migration concern
      And repository snapshot "pre-preview" is captured
      When tanren-cli upgrade preview runs
      Then the upgrade preview command succeeds
      And the upgrade preview is reported
      And the upgrade command requests confirmation
      And the upgrade preview lists compatibility concern "destructive-asset-changes"
      And the upgrade preview lists path "profiles/rust-cargo/legacy/renamed-standard.md"
      And no files are written in the repository fixture
      And the repository matches snapshot "pre-preview"

    # rationale: CLI is the command under proof while api-tag evidence shares the same behavior text.
    @positive @cli @api
    Scenario: Confirmed apply updates generated assets and preserves user-owned files
      Given a clean repository fixture
      And an installed repository fixture snapshot "v1" with profile "rust-cargo" and integrations "codex"
      And repository file ".codex/skills/plan-product.md" contains "legacy generated codex command"
      And repository file ".codex/skills/plan-product.md" baseline is recorded
      And repository file "profiles/rust-cargo/global/dependency-management.md" contains "team-owned dependency policy"
      And repository file "profiles/rust-cargo/global/dependency-management.md" baseline is recorded
      When tanren-cli upgrade apply runs with confirmation
      Then the upgrade apply command succeeds
      And repository file ".codex/skills/plan-product.md" is replaced from its baseline content
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content

    # rationale: CLI is the command under proof while mcp-tag evidence shares the same behavior text.
    @positive @cli @mcp
    Scenario: Preview reports migration concern before apply for CLI-first MCP coverage
      Given a clean repository fixture
      And an installed repository fixture snapshot "v1" with profile "rust-cargo" and integrations "codex"
      And legacy standards path "profiles/rust-cargo/legacy/renamed-standard.md" is marked as a migration concern
      When tanren-cli upgrade preview runs
      Then the upgrade preview command succeeds
      And the upgrade preview is reported
      And the upgrade preview lists compatibility concern "destructive-asset-changes"

    # rationale: CLI is the command under proof while tui-tag evidence shares the same behavior text.
    @positive @cli @tui
    Scenario: Confirmed apply can proceed after migration concern preview
      Given a clean repository fixture
      And an installed repository fixture snapshot "v1" with profile "rust-cargo" and integrations "codex"
      And legacy standards path "profiles/rust-cargo/legacy/renamed-standard.md" is marked as a migration concern
      When tanren-cli upgrade apply runs with confirmation
      Then the upgrade apply command succeeds
      And repository file "profiles/rust-cargo/legacy/renamed-standard.md" does not exist

    # rationale: CLI is the command under proof while web-tag falsification evidence shares the same behavior text.
    @falsification @cli @web
    Scenario: Preview reports no-install-manifest concern on repository without prior install
      Given a clean repository fixture
      And repository snapshot "empty" is captured
      When tanren-cli upgrade preview runs
      Then the upgrade preview command succeeds
      And the upgrade preview is reported
      And the upgrade command requests confirmation
      And the upgrade preview lists compatibility concern "no-install-manifest"
      And no files are written in the repository fixture
      And the repository matches snapshot "empty"

    # rationale: CLI is the command under proof while api-tag falsification evidence shares the same behavior text.
    @falsification @cli @api
    Scenario: Confirmed apply reports noop when install manifest is missing
      Given a clean repository fixture
      And repository snapshot "empty" is captured
      When tanren-cli upgrade apply runs with confirmation
      Then the upgrade apply command reports noop
      And no files are written in the repository fixture
      And the repository matches snapshot "empty"

    # rationale: CLI is the command under proof while mcp-tag falsification evidence shares the same behavior text.
    @falsification @cli @mcp
    Scenario: No-confirm upgrade leaves repository unchanged without prior install
      Given a clean repository fixture
      And repository snapshot "empty" is captured
      When tanren-cli upgrade preview runs
      Then the upgrade preview command succeeds
      And the upgrade command requests confirmation
      And no files are written in the repository fixture
      And the repository matches snapshot "empty"

    # rationale: CLI is the command under proof while tui-tag falsification evidence shares the same behavior text.
    @falsification @cli @tui
    Scenario: Missing-install apply remains a noop after preview
      Given a clean repository fixture
      And repository snapshot "empty" is captured
      When tanren-cli upgrade preview runs
      Then the upgrade preview command succeeds
      When tanren-cli upgrade apply runs with confirmation
      Then the upgrade apply command reports noop
      And no files are written in the repository fixture
      And the repository matches snapshot "empty"
