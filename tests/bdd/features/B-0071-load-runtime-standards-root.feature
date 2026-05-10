@B-0071
Feature: Load runtime standards from the configured repository root
  A builder can inspect installed standards through `tanren-cli standards inspect`
  using the standards root configured for the repository.

  @positive @cli
  Scenario: Standards inspect reports configured standards root and parsed standards after rust-cargo install
    Given a clean repository fixture
    When tanren-cli install runs with profile "rust-cargo"
    Then the install command succeeds
    And rust-cargo profile standards files are installed
    When tanren-cli standards inspect runs
    Then install command succeeds
    And standards inspect stdout reports configured standards metadata

  @positive @cli
  Scenario: Standards inspect reflects a relocated non-default configured standards root
    Given a clean repository fixture
    When tanren-cli install runs with profile "rust-cargo"
    Then the install command succeeds
    And rust-cargo profile standards files are installed
    Given standards assets are moved to repository path "profiles/rust-cargo-relocated"
    When tanren-cli standards inspect runs
    Then install command succeeds
    And standards inspect stdout reports configured standards metadata
    And repository file "profiles/rust-cargo-relocated/global/dependency-management.md" exists
    And repository file "profiles/rust-cargo/global/dependency-management.md" does not exist

  @falsification @cli
  Scenario: Standards inspect exits nonzero when configured standards root is missing
    Given a clean repository fixture
    When tanren-cli install runs with profile "rust-cargo"
    Then the install command succeeds
    Given the configured standards directory is deleted from the repository fixture
    When tanren-cli standards inspect runs
    Then the install command exits nonzero
    And the standards inspect stderr reports missing standards

  @falsification @cli
  Scenario: Standards inspect exits nonzero with a parse error naming the malformed standards file
    Given a clean repository fixture
    When tanren-cli install runs with profile "rust-cargo"
    Then the install command succeeds
    Given an installed standards markdown file is corrupted
    When tanren-cli standards inspect runs
    Then the install command exits nonzero
    And the standards inspect stderr reports standards parse failure
