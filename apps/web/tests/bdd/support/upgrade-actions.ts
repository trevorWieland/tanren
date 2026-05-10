/**
 * Shared fixture action names and payload builders for the BDD upgrade
 * witness harness. These live in test-only support so the production
 * account client never exports raw repository fixture actions.
 */

export type UpgradeFixtureAction =
  | "reset"
  | "write-file"
  | "record-baseline"
  | "seed-install"
  | "mark-legacy-migration-concern"
  | "capture-snapshot"
  | "run-upgrade-preview"
  | "run-upgrade-apply"
  | "assert-no-writes"
  | "assert-matches-snapshot"
  | "assert-preserves-baseline"
  | "assert-replaced-from-baseline"
  | "assert-file-missing"
  | "last-run";

export const ALL_FIXTURE_ACTIONS: readonly UpgradeFixtureAction[] = [
  "reset",
  "write-file",
  "record-baseline",
  "seed-install",
  "mark-legacy-migration-concern",
  "capture-snapshot",
  "run-upgrade-preview",
  "run-upgrade-apply",
  "assert-no-writes",
  "assert-matches-snapshot",
  "assert-preserves-baseline",
  "assert-replaced-from-baseline",
  "assert-file-missing",
  "last-run",
] as const;
