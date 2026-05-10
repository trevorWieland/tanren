import type {
  CommandResult,
  UpgradeApplyOutcomeLabel,
  UpgradeWorld,
} from "./asset-fixture";

export function assertUpgradeApplySuccess(world: UpgradeWorld): void {
  const run = assertSuccess(world.lastRun, "upgrade apply");
  assertIncludes(run.stdout, "status=ok command=upgrade");
  assertIncludes(run.stdout, "confirm=true applied=true");
  assertStructuredApplyOutcome(run, "applied");
  if (!run.applyOutcome?.report) {
    throw new Error("expected apply outcome to include a report summary");
  }
  const report = run.applyOutcome.report;
  if (!Array.isArray(report.created)) {
    throw new Error("expected report.created to be an array");
  }
  if (!Array.isArray(report.updated)) {
    throw new Error("expected report.updated to be an array");
  }
  if (!Array.isArray(report.removed)) {
    throw new Error("expected report.removed to be an array");
  }
  if (!Array.isArray(report.restored)) {
    throw new Error("expected report.restored to be an array");
  }
  if (!Array.isArray(report.preserved)) {
    throw new Error("expected report.preserved to be an array");
  }
}

export function assertUpgradeApplyNoop(world: UpgradeWorld): void {
  const run = assertSuccess(world.lastRun, "upgrade apply");
  assertIncludes(run.stdout, "status=noop command=upgrade");
  assertIncludes(run.stdout, "confirm=true applied=false");
  assertStructuredApplyOutcome(run, "no_manifest_noop");
}

export function assertUpgradePreviewSuccess(world: UpgradeWorld): void {
  assertSuccess(world.lastRun, "upgrade preview");
}

export function assertUpgradePreviewReported(world: UpgradeWorld): void {
  const run = requireLastRun(world);
  assertIncludes(run.stdout, "status=preview command=upgrade");
  assertIncludes(run.stdout, "preview changed=[");
  assertIncludes(run.stdout, "destructive=[");
  assertIncludes(run.stdout, "restored=[");
  assertIncludes(run.stdout, "removed=[");
  assertIncludes(run.stdout, "preserved=[");
  assertIncludes(run.stdout, "concerns=[");
}

export function assertUpgradeRequiresConfirmation(world: UpgradeWorld): void {
  const run = requireLastRun(world);
  assertIncludes(run.stdout, "status=confirmation_required command=upgrade");
  assertIncludes(run.stdout, "confirm=false applied=false");
}

export function assertPreviewContainsConcern(
  world: UpgradeWorld,
  concern: string,
): void {
  const run = requireLastRun(world);
  assertIncludes(run.stdout, concern);
}

export function assertPreviewContainsPath(
  world: UpgradeWorld,
  path: string,
): void {
  const run = requireLastRun(world);
  assertIncludes(run.stdout, path);
}

function requireLastRun(world: UpgradeWorld): CommandResult {
  if (!world.lastRun) {
    throw new Error("upgrade command has not been executed");
  }
  return world.lastRun;
}

function assertSuccess(
  run: CommandResult | undefined,
  command: string,
): CommandResult {
  if (!run || !run.success) {
    const status = run?.status ?? -1;
    throw new Error(`${command} failed with status ${status}`);
  }
  return run;
}

function assertIncludes(haystack: string, needle: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(`expected output to contain '${needle}'`);
  }
}

function assertStructuredApplyOutcome(
  run: CommandResult,
  expected: UpgradeApplyOutcomeLabel,
): void {
  if (!run.applyOutcome) {
    throw new Error(
      `expected structured apply outcome '${expected}' but found none`,
    );
  }
  if (run.applyOutcome.outcome !== expected) {
    throw new Error(
      `expected upgrade apply outcome '${expected}' but found '${run.applyOutcome.outcome}'`,
    );
  }
}
