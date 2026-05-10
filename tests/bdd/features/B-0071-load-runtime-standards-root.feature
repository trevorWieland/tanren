@B-0071
Feature: Load runtime standards from controlled projection and effective configuration
  A builder can inspect controlled standards projection assets through
  `tanren-cli standards inspect` using the repository's projected effective
  configuration.

  @positive @cli
  Scenario: Standards inspect reports effective configuration and parsed standards after rust-cargo projection install
    Given a clean repository fixture
    When tanren-cli install runs with profile "rust-cargo"
    Then the install command succeeds
    And rust-cargo profile standards projection files are installed
    And the repo methodology config projection matches the effective-configuration fixture
    When tanren-cli standards inspect runs
    Then install command succeeds
    And standards inspect stdout reports effective configuration metadata
    And no files are written in the repository fixture

  @positive @cli
  Scenario: Standards inspect reflects a relocated non-default projected standards root
    Given a clean repository fixture
    When tanren-cli install runs with profile "rust-cargo"
    Then the install command succeeds
    And rust-cargo profile standards projection files are installed
    And the repo methodology config projection matches the effective-configuration fixture
    Given standards projection assets are moved to repository path "profiles/rust-cargo-relocated"
    When tanren-cli standards inspect runs
    Then install command succeeds
    And standards inspect stdout reports effective configuration metadata
    And no files are written in the repository fixture
    And repository file "profiles/rust-cargo-relocated/global/dependency-management.md" exists
    And repository file "profiles/rust-cargo/global/dependency-management.md" does not exist
    And the repo methodology config projection matches the effective-configuration fixture

  @falsification @cli
  Scenario: Standards inspect exits nonzero when configured standards projection root is missing
    Given a clean repository fixture
    When tanren-cli install runs with profile "rust-cargo"
    Then the install command succeeds
    Given the configured standards projection directory is deleted from the repository fixture
    When tanren-cli standards inspect runs
    Then the install command exits nonzero
    And the standards inspect stderr reports missing configured standards projection
    And no files are written in the repository fixture

  @falsification @cli
  Scenario: Standards inspect exits nonzero with malformed standards projection frontmatter
    Given a clean repository fixture
    When tanren-cli install runs with profile "rust-cargo"
    Then the install command succeeds
    Given an installed standards projection markdown file is corrupted
    When tanren-cli standards inspect runs
    Then the install command exits nonzero
    And the standards inspect stderr reports malformed standards projection frontmatter
    And no files are written in the repository fixture
