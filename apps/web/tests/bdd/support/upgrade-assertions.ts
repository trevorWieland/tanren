import type {
  CommandResult,
  UpgradeApplyOutcomeLabel,
  UpgradeWorld,
} from "./asset-fixture";

export function assertUpgradeApplySuccess(world: UpgradeWorld): void {
  const run = assertSuccess(world.lastRun, "upgrade apply");
  assertIncludes(run.stdout, "status=ok command=upgrade");
  assertIncludes(run.stdout, "confirm=true applied=true");
  assertUpgradeApplyOutcome(run, "applied");
  assertIncludes(run.stdout, "applied created=[");
  assertIncludes(run.stdout, "updated=[");
  assertIncludes(run.stdout, "removed=[");
  assertIncludes(run.stdout, "restored=[");
  assertIncludes(run.stdout, "preserved=[");
}

export function assertUpgradeApplyNoop(world: UpgradeWorld): void {
  const run = assertSuccess(world.lastRun, "upgrade apply");
  assertIncludes(run.stdout, "status=noop command=upgrade");
  assertIncludes(run.stdout, "confirm=true applied=false");
  assertUpgradeApplyOutcome(run, "no_manifest_noop");
}

export function assertUpgradePreviewSuccess(world: UpgradeWorld): void {
  assertSuccess(world.lastRun, "upgrade preview");
}

export function assertUpgradePreviewReported(world: UpgradeWorld): void {
  const run = requireLastRun(world);
  assertIncludes(run.stdout, "status=preview command=upgrade");
  assertIncludes(run.stdout, "preview changed=[");
  assertIncludes(run.stdout, "destructive=[");
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

function assertUpgradeApplyOutcome(
  run: CommandResult,
  expected: UpgradeApplyOutcomeLabel,
): void {
  if (run.upgradeApplyOutcome !== expected) {
    throw new Error(
      `expected upgrade apply outcome '${expected}' but found '${run.upgradeApplyOutcome ?? "undefined"}'`,
    );
  }
}
