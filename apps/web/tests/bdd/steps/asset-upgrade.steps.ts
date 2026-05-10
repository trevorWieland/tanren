/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0134.
//
// The shared Gherkin in `tests/bdd/features/B-0134-upgrade-installed-tanren-assets.feature`
// remains the single source of truth for Rust and Playwright runners.

import { spawn } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";

import {
  type PlaywrightTestArgs,
  type PlaywrightTestOptions,
  type PlaywrightWorkerArgs,
  type PlaywrightWorkerOptions,
  type TestType,
} from "@playwright/test";
import {
  type BddTestFixtures,
  type BddWorkerFixtures,
  createBdd,
  test as base,
} from "playwright-bdd";
import {
  type RepositorySnapshot,
  sha256Hex,
  snapshotRepositoryForUpgradeAssertions,
} from "../support/asset-fixture";

interface CommandResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly status: number;
  readonly success: boolean;
}

interface UpgradeWorld {
  repositoryRoot?: string;
  labeledSnapshots: Map<string, RepositorySnapshot>;
  fileBaselines: Map<string, Uint8Array>;
  trackedSnapshotPaths: Set<string>;
  snapshotBeforeLastRun?: RepositorySnapshot;
  lastRun?: CommandResult;
}

type UpgradeTest = TestType<
  PlaywrightTestArgs &
    PlaywrightTestOptions &
    BddTestFixtures & { world: UpgradeWorld },
  PlaywrightWorkerArgs & PlaywrightWorkerOptions & BddWorkerFixtures
>;

export const test: UpgradeTest = base.extend<{ world: UpgradeWorld }>({
  world: async ({}, use) => {
    await use({
      labeledSnapshots: new Map<string, RepositorySnapshot>(),
      fileBaselines: new Map<string, Uint8Array>(),
      trackedSnapshotPaths: new Set<string>(),
    });
  },
});

const { Given, When, Then } = createBdd(test);

Given("a clean repository fixture", async ({ world }) => {
  if (world.repositoryRoot) {
    await rm(world.repositoryRoot, { recursive: true, force: true });
  }
  world.repositoryRoot = await mkdtemp(
    join(tmpdir(), "tanren-web-bdd-install-"),
  );
  world.labeledSnapshots.clear();
  world.fileBaselines.clear();
  world.trackedSnapshotPaths.clear();
  delete world.snapshotBeforeLastRun;
  delete world.lastRun;
});

Given(
  /^repository file "([^"]+)" contains "([^"]+)"$/,
  async ({ world }, path: string, content: string) => {
    const repositoryRoot = requireRepositoryRoot(world);
    const relativePath = parseRelativePath(path);
    trackScenarioOwnedPath(world, relativePath);
    const absolutePath = resolve(repositoryRoot, relativePath);
    await mkdir(dirname(absolutePath), { recursive: true });
    await writeFile(absolutePath, content, "utf-8");
  },
);

Given(
  /^repository file "([^"]+)" baseline is recorded$/,
  async ({ world }, path: string) => {
    const repositoryRoot = requireRepositoryRoot(world);
    const relativePath = parseRelativePath(path);
    trackScenarioOwnedPath(world, relativePath);
    const absolutePath = resolve(repositoryRoot, relativePath);
    const baseline = await readFile(absolutePath);
    world.fileBaselines.set(relativePath, Uint8Array.from(baseline));
  },
);

When("tanren-cli upgrade apply runs with confirmation", async ({ world }) => {
  const repositoryRoot = requireRepositoryRoot(world);
  await runUpgradeCommand(world, [
    "upgrade",
    "--repo",
    repositoryRoot,
    "--confirm",
  ]);
});

Then("the upgrade apply command succeeds", async ({ world }) => {
  assertSuccess(world.lastRun, "upgrade apply");
  const run = requireLastRun(world);
  assertIncludes(run.stdout, "status=ok command=upgrade");
  assertIncludes(run.stdout, "confirm=true applied=true");
  assertIncludes(run.stdout, "applied created=[");
  assertIncludes(run.stdout, "updated=[");
  assertIncludes(run.stdout, "removed=[");
  assertIncludes(run.stdout, "restored=[");
  assertIncludes(run.stdout, "preserved=[");
});

Then("the upgrade apply command reports noop", async ({ world }) => {
  assertSuccess(world.lastRun, "upgrade apply");
  const run = requireLastRun(world);
  assertIncludes(run.stdout, "status=noop command=upgrade");
  assertIncludes(run.stdout, "confirm=true applied=false");
});

Then(
  /^repository file "([^"]+)" preserves its baseline content$/,
  async ({ world }, path: string) => {
    const repositoryRoot = requireRepositoryRoot(world);
    const relativePath = parseRelativePath(path);
    const baseline = world.fileBaselines.get(relativePath);
    if (!baseline) {
      throw new Error(`missing baseline for '${relativePath}'`);
    }
    const absolutePath = resolve(repositoryRoot, relativePath);
    const current = await readFile(absolutePath);
    if (!byteArraysEqual(current, baseline)) {
      throw new Error(
        `expected '${relativePath}' to preserve baseline content`,
      );
    }
  },
);

Then(
  /^repository file "([^"]+)" is replaced from its baseline content$/,
  async ({ world }, path: string) => {
    const repositoryRoot = requireRepositoryRoot(world);
    const relativePath = parseRelativePath(path);
    const baseline = world.fileBaselines.get(relativePath);
    if (!baseline) {
      throw new Error(`missing baseline for '${relativePath}'`);
    }
    const absolutePath = resolve(repositoryRoot, relativePath);
    const current = await readFile(absolutePath);
    if (byteArraysEqual(current, baseline)) {
      throw new Error(`expected '${relativePath}' to differ from baseline`);
    }
  },
);

Then(
  /^repository file "([^"]+)" does not exist$/,
  async ({ world }, path: string) => {
    const repositoryRoot = requireRepositoryRoot(world);
    const relativePath = parseRelativePath(path);
    const absolutePath = resolve(repositoryRoot, relativePath);
    try {
      await readFile(absolutePath);
      throw new Error(`expected '${relativePath}' to be absent`);
    } catch (error) {
      const enoent =
        typeof error === "object" &&
        error !== null &&
        "code" in error &&
        (error as { code?: string }).code === "ENOENT";
      if (!enoent) {
        throw error;
      }
    }
  },
);

Given(
  /^an installed repository fixture snapshot "([^"]+)" with profile "([^"]+)" and integrations "([^"]+)"$/,
  async (
    { world },
    snapshotLabel: string,
    profile: string,
    integrations: string,
  ) => {
    const repositoryRoot = requireRepositoryRoot(world);
    await runUpgradeCommand(world, [
      "install",
      "--repo",
      repositoryRoot,
      "--profile",
      profile,
      "--integrations",
      integrations,
    ]);
    assertSuccess(world.lastRun, "install");
    await captureSnapshot(world, snapshotLabel);
  },
);

Given(
  /^legacy standards path "([^"]+)" is marked as a migration concern$/,
  async ({ world }, path: string) => {
    const repositoryRoot = requireRepositoryRoot(world);
    const relativePath = parseRelativePath(path);
    trackScenarioOwnedPath(world, relativePath);
    const absolutePath = resolve(repositoryRoot, relativePath);
    await mkdir(resolve(absolutePath, ".."), { recursive: true });
    const legacyContent = "legacy standards asset requiring migration";
    await writeFile(absolutePath, legacyContent, "utf-8");

    const manifestPath = resolve(
      repositoryRoot,
      ".tanren/install-manifest.toml",
    );
    const manifestRaw = await readFile(manifestPath, "utf-8");
    const contentHash = sha256Hex(legacyContent);
    const staleEntry =
      `\n[[entries]]\n` +
      `path = "${relativePath}"\n` +
      `content_hash = "${contentHash}"\n` +
      'asset_class = "methodology-command"\n' +
      'integration = "codex"\n' +
      'preservation = "replace-generated"\n';
    await writeFile(manifestPath, `${manifestRaw}${staleEntry}`, "utf-8");
  },
);

Given(
  /^repository snapshot "([^"]+)" is captured$/,
  async ({ world }, label: string) => {
    await captureSnapshot(world, label);
  },
);

When("tanren-cli upgrade preview runs", async ({ world }) => {
  const repositoryRoot = requireRepositoryRoot(world);
  await runUpgradeCommand(world, ["upgrade", "--repo", repositoryRoot]);
});

Then("the upgrade preview command succeeds", async ({ world }) => {
  assertSuccess(world.lastRun, "upgrade preview");
});

Then("the upgrade preview is reported", async ({ world }) => {
  const run = requireLastRun(world);
  assertIncludes(run.stdout, "status=preview command=upgrade");
  assertIncludes(run.stdout, "preview changed=[");
  assertIncludes(run.stdout, "destructive=[");
  assertIncludes(run.stdout, "preserved=[");
  assertIncludes(run.stdout, "concerns=[");
});

Then("the upgrade command requests confirmation", async ({ world }) => {
  const run = requireLastRun(world);
  assertIncludes(run.stdout, "status=confirmation_required command=upgrade");
  assertIncludes(run.stdout, "confirm=false applied=false");
});

Then(
  /^the upgrade preview lists compatibility concern "([^"]+)"$/,
  async ({ world }, concern: string) => {
    const run = requireLastRun(world);
    assertIncludes(run.stdout, concern);
  },
);

Then(
  /^the upgrade preview lists path "([^"]+)"$/,
  async ({ world }, path: string) => {
    const run = requireLastRun(world);
    assertIncludes(run.stdout, parseRelativePath(path));
  },
);

Then("no files are written in the repository fixture", async ({ world }) => {
  if (!world.snapshotBeforeLastRun) {
    throw new Error("missing snapshot before last command run");
  }
  const after = await captureScopedSnapshot(world);
  if (!snapshotsEqual(after, world.snapshotBeforeLastRun)) {
    throw new Error("repository changed but expected no writes");
  }
});

Then(
  /^the repository matches snapshot "([^"]+)"$/,
  async ({ world }, label: string) => {
    const expected = world.labeledSnapshots.get(label);
    if (!expected) {
      throw new Error(`missing labeled snapshot '${label}'`);
    }
    const actual = await captureScopedSnapshot(world);
    if (!snapshotsEqual(actual, expected)) {
      throw new Error(`repository does not match labeled snapshot '${label}'`);
    }
  },
);

function requireRepositoryRoot(world: UpgradeWorld): string {
  if (!world.repositoryRoot) {
    throw new Error("repository fixture is not initialized");
  }
  return world.repositoryRoot;
}

function requireLastRun(world: UpgradeWorld): CommandResult {
  if (!world.lastRun) {
    throw new Error("upgrade command has not been executed");
  }
  return world.lastRun;
}

function assertSuccess(run: CommandResult | undefined, command: string): void {
  if (!run || !run.success) {
    const status = run?.status ?? -1;
    const stderr = run?.stderr ?? "";
    throw new Error(`${command} failed with status ${status}: ${stderr}`);
  }
}

function assertIncludes(haystack: string, needle: string): void {
  if (!haystack.includes(needle)) {
    throw new Error(`expected output to contain '${needle}'`);
  }
}

function parseRelativePath(raw: string): string {
  const value = raw.trim();
  if (value.length === 0 || isAbsolute(value) || value.includes("\\")) {
    throw new Error(`invalid repository-relative path '${raw}'`);
  }
  const segments = value.split("/");
  for (const segment of segments) {
    if (segment.length === 0 || segment === "." || segment === "..") {
      throw new Error(`invalid repository-relative path '${raw}'`);
    }
  }
  return value;
}

async function captureSnapshot(
  world: UpgradeWorld,
  label: string,
): Promise<void> {
  const trimmed = label.trim();
  if (trimmed.length === 0) {
    throw new Error("snapshot label cannot be empty");
  }
  world.labeledSnapshots.set(trimmed, await captureScopedSnapshot(world));
}

async function runUpgradeCommand(
  world: UpgradeWorld,
  installArgs: readonly string[],
): Promise<void> {
  const workspaceRoot = resolveWorkspaceRoot();
  const before = await captureScopedSnapshot(world);
  const result = await runCommand(
    "cargo",
    ["run", "-q", "-p", "tanren-cli", "--", ...installArgs],
    workspaceRoot,
  );
  world.snapshotBeforeLastRun = before;
  world.lastRun = result;
}

async function captureScopedSnapshot(
  world: UpgradeWorld,
): Promise<RepositorySnapshot> {
  const repositoryRoot = requireRepositoryRoot(world);
  return await snapshotRepositoryForUpgradeAssertions(
    repositoryRoot,
    resolveWorkspaceRoot(),
    world.trackedSnapshotPaths,
  );
}

function resolveWorkspaceRoot(): string {
  return process.env["TANREN_REPO_ROOT"] ?? resolve(process.cwd(), "..", "..");
}

function trackScenarioOwnedPath(
  world: UpgradeWorld,
  relativePath: string,
): void {
  world.trackedSnapshotPaths.add(relativePath);
}

async function runCommand(
  cmd: string,
  args: readonly string[],
  cwd: string,
): Promise<CommandResult> {
  return await new Promise<CommandResult>((resolvePromise, rejectPromise) => {
    const child = spawn(cmd, [...args], {
      cwd,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (chunk: Buffer | string) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk: Buffer | string) => {
      stderr += chunk.toString();
    });

    child.on("error", (error) => {
      rejectPromise(error);
    });

    child.on("close", (code) => {
      resolvePromise({
        stdout,
        stderr,
        status: code ?? 1,
        success: code === 0,
      });
    });
  });
}

function snapshotsEqual(
  left: RepositorySnapshot,
  right: RepositorySnapshot,
): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function byteArraysEqual(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) {
    return false;
  }
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return false;
    }
  }
  return true;
}
