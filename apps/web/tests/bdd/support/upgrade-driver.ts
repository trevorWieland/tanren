// Upgrade fixture driver — calls the test-hooks API directly from Node
// instead of navigating to a browser witness page. The test-hook secret
// (TANREN_TEST_HOOK_SECRET) is injected into the Playwright driver
// process env by global-setup.ts and never projected into the Next.js
// client bundle.

import {
  type CommandResult,
  type FixtureActionName,
  type FixtureActionResult,
  type UpgradeWorld,
} from "./asset-fixture";

function apiUrl(): string {
  const url = process.env["NEXT_PUBLIC_API_URL"];
  if (!url) {
    throw new Error("NEXT_PUBLIC_API_URL is not set");
  }
  return url;
}

function testHookHeaders(): Record<string, string> {
  const secret = process.env["TANREN_TEST_HOOK_SECRET"];
  const headers: Record<string, string> = {
    "content-type": "application/json",
  };
  if (secret) {
    headers["x-test-hook-secret"] = secret;
  }
  return headers;
}

async function execute(
  action: FixtureActionName,
  payload: unknown,
): Promise<FixtureActionResult> {
  const res = await fetch(`${apiUrl()}/test-hooks/upgrade-fixture/${action}`, {
    method: "POST",
    headers: testHookHeaders(),
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(
      `upgrade fixture action '${action}' failed: ${res.status} ${text}`,
    );
  }
  const result = (await res.json()) as Partial<FixtureActionResult>;
  if (result.ok !== true) {
    throw new Error(
      `upgrade fixture action '${action}' returned error: ${JSON.stringify(result)}`,
    );
  }
  return result as FixtureActionResult;
}

function parseCommandResult(result: FixtureActionResult): CommandResult {
  const stdout = expectString(result.stdout, "stdout");
  const commandResult: CommandResult = {
    stdout,
    status: expectNumber(result.status, "status"),
    success: expectBoolean(result.success, "success"),
  };
  if (result.apply_outcome) {
    return { ...commandResult, applyOutcome: result.apply_outcome };
  }
  return commandResult;
}

export class UpgradeDriver {
  async resetFixture(): Promise<void> {
    await execute("reset", {});
  }

  async writeRepositoryFile(path: string, content: string): Promise<void> {
    await execute("write-file", { path, content });
  }

  async recordBaseline(path: string): Promise<void> {
    await execute("record-baseline", { path });
  }

  async seedInstalledSnapshot(
    snapshotLabel: string,
    profile: string,
    integrations: string,
  ): Promise<void> {
    await execute("seed-install", {
      snapshot_label: snapshotLabel,
      profile,
      integrations,
    });
  }

  async markLegacyMigrationConcern(path: string): Promise<void> {
    await execute("mark-legacy-migration-concern", { path });
  }

  async captureSnapshot(label: string): Promise<void> {
    await execute("capture-snapshot", { label });
  }

  async runUpgradePreview(world: UpgradeWorld): Promise<void> {
    const result = await execute("run-upgrade-preview", {});
    world.lastRun = parseCommandResult(result);
  }

  async runUpgradeApply(world: UpgradeWorld): Promise<void> {
    const result = await execute("run-upgrade-apply", {});
    world.lastRun = parseCommandResult(result);
  }

  async assertNoWrites(): Promise<void> {
    await execute("assert-no-writes", {});
  }

  async assertMatchesSnapshot(label: string): Promise<void> {
    await execute("assert-matches-snapshot", { label });
  }

  async assertPreservesBaseline(path: string): Promise<void> {
    await execute("assert-preserves-baseline", { path });
  }

  async assertReplacedFromBaseline(path: string): Promise<void> {
    await execute("assert-replaced-from-baseline", { path });
  }

  async assertFileMissing(path: string): Promise<void> {
    await execute("assert-file-missing", { path });
  }

  async assertFileContains(path: string, content: string): Promise<void> {
    await execute("assert-file-contains", { path, content });
  }

  async assertFileNotContains(path: string, content: string): Promise<void> {
    await execute("assert-file-not-contains", { path, content });
  }
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new Error(`expected ${label} to be a string`);
  }
  return value;
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== "number") {
    throw new Error(`expected ${label} to be a number`);
  }
  return value;
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`expected ${label} to be a boolean`);
  }
  return value;
}
