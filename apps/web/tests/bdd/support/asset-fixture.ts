export type { UpgradeFixtureAction } from "./upgrade-actions";

export type UpgradeApplyOutcomeLabel =
  | "applied"
  | "no_manifest_noop"
  | "blocked";

export interface ApplyReportSummary {
  readonly created: readonly string[];
  readonly updated: readonly string[];
  readonly removed: readonly string[];
  readonly restored: readonly string[];
  readonly preserved: readonly string[];
}

export interface UpgradeApplyOutcome {
  readonly outcome: UpgradeApplyOutcomeLabel;
  readonly report?: ApplyReportSummary;
  readonly reason?: string;
}

export interface FixtureActionResult {
  readonly ok: boolean;
  readonly stdout?: string;
  readonly status?: number;
  readonly success?: boolean;
  readonly apply_outcome?: UpgradeApplyOutcome;
}

export type FixtureActionName =
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

export interface CommandResult {
  readonly stdout: string;
  readonly status: number;
  readonly success: boolean;
  readonly applyOutcome?: UpgradeApplyOutcome;
}

export interface UpgradeWorld {
  lastRun?: CommandResult;
}
