export interface FixtureActionResult {
  readonly ok: boolean;
  readonly stdout?: string;
  readonly status?: number;
  readonly success?: boolean;
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
}

export interface UpgradeWorld {
  lastRun?: CommandResult;
}
