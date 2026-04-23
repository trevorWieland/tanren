@phase0 @wave_c @feature7
Feature: Feature 7 installer and self-hosting are deterministic

  @positive @BEH-P0-701
  Scenario: 7.1 positive witness - install output is predictable and idempotent
    Given a configured repository
    When install is run repeatedly without source/config change
    Then first run renders targets and later runs are no-op

  @falsification @BEH-P0-701
  Scenario: 7.1 falsification witness - changed source/config is not a no-op rerun
    Given install source or configuration changed since last render
    When install is run again after source/config change
    Then rerun is not treated as a no-op
    And rerun converges deterministically to updated outputs

  @positive @BEH-P0-702
  Scenario: 7.2 positive witness - drift is detectable and explicit
    Given rendered outputs diverge from source-of-truth templates
    When strict dry-run install is executed
    Then drift is reported and process fails explicitly
    And strict dry-run performs no mutation

  @falsification @BEH-P0-702
  Scenario: 7.2 falsification witness - no drift means strict dry-run succeeds
    Given rendered outputs match source-of-truth templates
    When strict dry-run install is executed
    Then no drift is reported and process succeeds
    And strict dry-run performs no mutation

  @positive @BEH-P0-703
  Scenario: 7.3 positive witness - multi-target render stays semantically aligned
    Given shared command source
    When artifacts are rendered for multiple frameworks
    Then framework-specific wrappers may differ
    And command intent and capability semantics remain equivalent

  @falsification @BEH-P0-703
  Scenario: 7.3 falsification witness - semantic drift is detected across targets
    Given multi-target render with semantic capability drift
    When cross-target parity is evaluated
    Then semantic drift is reported
    And targets are not considered aligned
