@B-0068
Feature: Bootstrap Tanren assets into an existing repository
  A builder can run `tanren-cli install` to bootstrap Tanren assets for a
  supported profile, and invalid inputs fail before any repository writes.

  Rule: CLI install and reinstall outcomes

    @positive @cli
    Scenario: Install provides default rust-cargo assets in an empty repository
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And the install output redacts absolute repository paths
      And rust-cargo defaults install all methodology command assets and standards files
      And future installs remember the rust-cargo profile and default integrations

    @falsification @cli
    Scenario: First install preserves pre-existing user-edited standards files
      Given a clean repository
      And repository file "profiles/rust-cargo/global/dependency-management.md" contains "custom organization dependency policy"
      And repository file "profiles/rust-cargo/global/dependency-management.md" baseline is recorded
      And repository file "profiles/rust-cargo/global/just-ci-gate.md" is seeded from workspace catalog
      And repository file "profiles/rust-cargo/global/just-ci-gate.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And rust-cargo defaults install all methodology command assets and standards files
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content
      And repository file "profiles/rust-cargo/global/just-ci-gate.md" preserves its baseline content

    @positive @cli
    Scenario: Reinstall refreshes generated assets while preserving standards edits
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file ".codex/skills/plan-product.md" contains "stale generated codex command"
      And repository file ".codex/skills/plan-product.md" baseline is recorded
      And repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And reinstall treats repository file ".codex/skills/retired-command.md" as an obsolete generated Tanren asset
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" baseline is recorded
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" is missing before install
      And repository file "profiles/rust-cargo/rust/type-safety-patterns.md" contains "team-edited standards content"
      And repository file "profiles/rust-cargo/rust/type-safety-patterns.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And the install output reports created, updated, removed, restored, and preserved summaries
      And repository file ".codex/skills/plan-product.md" is replaced from its baseline content
      And obsolete generated file ".codex/skills/retired-command.md" is removed during reinstall
      And repository file "profiles/rust-cargo/testing/mock-boundaries.md" preserves its baseline content
      And repository file "profiles/rust-cargo/rust/type-safety-patterns.md" preserves its baseline content

    @falsification @cli
    Scenario: Install fails fast for an unknown profile
      Given a clean repository
      When tanren-cli install runs with profile "not-a-profile"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the repository remains unchanged

    @falsification @cli
    Scenario: Install blocks command writes through directory symlink paths
      Given a clean repository
      And repository path ".codex/skills" is a directory symlink to "external-codex-skills"
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the install output redacts absolute repository paths
      And the install stderr contains "repository path '.codex/skills/"

    @falsification @cli
    Scenario: Install blocks generated command writes through file symlink paths
      Given a clean repository
      And repository file "outside-command.md" contains "external command file"
      And repository path ".codex/skills/plan-product.md" is a file symlink to "outside-command.md"
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the install stderr contains "repository path '.codex/skills/plan-product.md'"
      And repository file "outside-command.md" contains "external command file"

    @falsification @cli
    Scenario: Reinstall blocks generated cleanup through file symlink paths
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command succeeds
      Given repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And reinstall treats repository file ".codex/skills/retired-command.md" as an obsolete generated Tanren asset
      And repository file "outside-remove-target.md" contains "external"
      And repository file "outside-remove-target.md" baseline is recorded
      And repository path ".codex/skills/retired-command.md" is a file symlink to "outside-remove-target.md"
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the install stderr contains "repository path '.codex/skills/retired-command.md'"
      And repository file "outside-remove-target.md" preserves its baseline content

    @falsification @cli
    Scenario: Reinstall leaves the repository unchanged when validation fails
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command succeeds
      Given repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And reinstall treats repository file ".codex/skills/retired-command.md" as an obsolete generated Tanren asset
      And repository file ".codex/skills/retired-command.md" baseline is recorded
      And repository file "outside-command.md" contains "external command file"
      And repository path ".codex/skills/plan-product.md" is a file symlink to "outside-command.md"
      When tanren-cli install runs with profile "rust-cargo" and integrations "codex"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the repository remains unchanged
      And repository file ".codex/skills/retired-command.md" preserves its baseline content
      And repository file "outside-command.md" contains "external command file"

    @falsification @cli
    Scenario: Reinstall does not overwrite user-edited standards content
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "profiles/rust-cargo/global/dependency-management.md" contains "custom organization dependency policy"
      And repository file "profiles/rust-cargo/global/dependency-management.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And repository file "profiles/rust-cargo/global/dependency-management.md" preserves its baseline content

    @falsification @cli
    Scenario: Reinstall does not remove unrelated repository files
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file "README.md" contains "repository-owned content"
      And repository file "README.md" baseline is recorded
      And reinstall treats repository file "README.md" as an obsolete generated Tanren asset
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And repository file "README.md" preserves its baseline content

    @falsification @cli
    Scenario: Reinstall fails when prior generated-path records are invalid
      Given a clean repository
      And repository file "README.md" contains "repository-owned content"
      And repository file "README.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given reinstall input includes an invalid generated asset path "../README.md"
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the repository remains unchanged
      And repository file "README.md" preserves its baseline content

    @falsification @cli
    Scenario: Reinstall fails when prior generated metadata is malformed
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given reinstall input includes malformed generated asset metadata
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the repository remains unchanged

    @falsification @cli
    Scenario: Reinstall preserves older generated files after user edits
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given repository file ".codex/skills/retired-command.md" contains "stale generated command from old manifest"
      And reinstall treats repository file ".codex/skills/retired-command.md" as an obsolete generated Tanren asset
      And repository file ".codex/skills/retired-command.md" contains "team-edited stale generated command"
      And repository file ".codex/skills/retired-command.md" baseline is recorded
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      And repository file ".codex/skills/retired-command.md" preserves its baseline content
    @falsification @cli
    Scenario: Install rejects manifest with CurDir-aliased duplicate entry paths
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given previous install manifest is tampered with raw generated path "./.codex/skills/plan-product.md"
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the repository remains unchanged

    @falsification @cli
    Scenario: Install rejects manifest entry with only CurDir segments
      Given a clean repository
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command succeeds
      Given previous install manifest is tampered with raw generated path "././."
      When tanren-cli install runs with profile "rust-cargo"
      Then the install command exits nonzero
      And the install output reports why install was blocked
      And the repository remains unchanged
