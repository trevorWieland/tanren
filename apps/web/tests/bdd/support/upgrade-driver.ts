import { expect, type Page } from "@playwright/test";

import {
  type CommandResult,
  type FixtureActionName,
  type FixtureActionResult,
  type UpgradeApplyOutcomeLabel,
  type UpgradeWorld,
} from "./asset-fixture";

const OUTPUT_LOCATOR = '[data-testid="upgrade-witness-output"]';

export class UpgradeDriver {
  readonly #page: Page;

  constructor(page: Page) {
    this.#page = page;
  }

  async resetFixture(): Promise<void> {
    await this.execute("reset", {});
  }

  async writeRepositoryFile(path: string, content: string): Promise<void> {
    await this.execute("write-file", { path, content });
  }

  async recordBaseline(path: string): Promise<void> {
    await this.execute("record-baseline", { path });
  }

  async seedInstalledSnapshot(
    snapshotLabel: string,
    profile: string,
    integrations: string,
  ): Promise<void> {
    await this.execute("seed-install", {
      snapshot_label: snapshotLabel,
      profile,
      integrations,
    });
  }

  async markLegacyMigrationConcern(path: string): Promise<void> {
    await this.execute("mark-legacy-migration-concern", { path });
  }

  async captureSnapshot(label: string): Promise<void> {
    await this.execute("capture-snapshot", { label });
  }

  async runUpgradePreview(world: UpgradeWorld): Promise<void> {
    world.lastRun = await this.executeCommand("run-upgrade-preview", {});
  }

  async runUpgradeApply(world: UpgradeWorld): Promise<void> {
    world.lastRun = await this.executeCommand("run-upgrade-apply", {});
  }

  async assertNoWrites(): Promise<void> {
    await this.execute("assert-no-writes", {});
  }

  async assertMatchesSnapshot(label: string): Promise<void> {
    await this.execute("assert-matches-snapshot", { label });
  }

  async assertPreservesBaseline(path: string): Promise<void> {
    await this.execute("assert-preserves-baseline", { path });
  }

  async assertReplacedFromBaseline(path: string): Promise<void> {
    await this.execute("assert-replaced-from-baseline", { path });
  }

  async assertFileMissing(path: string): Promise<void> {
    await this.execute("assert-file-missing", { path });
  }

  async openHarness(): Promise<void> {
    await this.#page.goto("/");
    await expect(
      this.#page.locator('[data-testid="upgrade-witness-action"]'),
    ).toBeVisible();
  }

  private async executeCommand(
    action: FixtureActionName,
    payload: unknown,
  ): Promise<CommandResult> {
    const result = await this.execute(action, payload);
    const stdout = expectString(result.stdout, "stdout");
    const commandResult: CommandResult = {
      stdout,
      status: expectNumber(result.status, "status"),
      success: expectBoolean(result.success, "success"),
    };
    const upgradeApplyOutcome = parseUpgradeApplyOutcome(stdout);
    if (upgradeApplyOutcome) {
      return { ...commandResult, upgradeApplyOutcome };
    }
    return commandResult;
  }

  private async execute(
    action: FixtureActionName,
    payload: unknown,
  ): Promise<FixtureActionResult> {
    await this.openHarness();

    await this.#page
      .locator('[data-testid="upgrade-witness-action"]')
      .selectOption(action);
    await this.#page
      .locator('[data-testid="upgrade-witness-payload"]')
      .fill(JSON.stringify(payload));

    await this.#page.locator('[data-testid="upgrade-witness-run"]').click();
    await expect(
      this.#page.locator('[data-testid="upgrade-witness-run"]'),
    ).toHaveText("run");

    const outputRaw =
      (await this.#page.locator(OUTPUT_LOCATOR).textContent())?.trim() ?? "";
    if (outputRaw === "") {
      throw new Error(
        `upgrade witness action '${action}' returned empty output`,
      );
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(outputRaw);
    } catch (reason: unknown) {
      throw new Error(
        `upgrade witness action '${action}' returned non-JSON output: ${String(
          reason,
        )}\n${outputRaw}`,
      );
    }
    const result = parsed as Partial<FixtureActionResult>;
    if (result.ok !== true) {
      throw new Error(
        `upgrade witness action '${action}' failed: ${JSON.stringify(result)}`,
      );
    }
    return result as FixtureActionResult;
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

function parseUpgradeApplyOutcome(
  stdout: string,
): UpgradeApplyOutcomeLabel | undefined {
  const lines = stdout.split("\n");
  for (const line of lines) {
    if (!line.includes(" command=upgrade ") || !line.includes(" outcome=")) {
      continue;
    }
    const match = line.match(/\boutcome=([a-z_]+)\b/);
    if (!match) {
      continue;
    }
    if (match[1] === "applied") {
      return "applied";
    }
    if (match[1] === "no_manifest_noop") {
      return "no_manifest_noop";
    }
    if (match[1] === "blocked") {
      return "blocked";
    }
    throw new Error(`unknown upgrade apply outcome label '${match[1]}'`);
  }
  return undefined;
}
