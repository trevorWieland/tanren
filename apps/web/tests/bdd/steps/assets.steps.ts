/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0134.
//
// The Gherkin in `tests/bdd/features/B-0134-upgrade-installed-tanren-assets.feature`
// is the single source of truth for both the Rust `tanren-bdd` runner and
// this Node `playwright-bdd` runner — the `apps/web/tests/bdd/features`
// path is a symlink into the canonical directory.
//
// Coverage:
//
// - Positive: preview shows actions/concerns; confirmed apply updates
//   generated assets while preserving user-owned paths.
// - Falsification: preview-only leaves the repository unchanged.
//
// State is held at module scope rather than a per-scenario fixture
// extension. Playwright-bdd runs scenarios sequentially, so this is
// safe and avoids a TS2742 type-portability issue with exported
// `test.extend()`.

import { createBdd, test as base } from "playwright-bdd";
import { createHash } from "crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "fs";
import { tmpdir } from "os";
import { join } from "path";

interface UpgradePreviewData {
  source_version: string;
  target_version: string;
  actions: Array<{ action: string; path: string }>;
  concerns: Array<{ kind: string; path: string; detail: string }>;
  preserved_user_paths: string[];
}

const { Given, When, Then } = createBdd(base);

let fixtureRoot: string | null = null;
let lastPreview: UpgradePreviewData | null = null;

const FIXTURE_SOURCE_VERSION = "0.1.0-fixture";

function computeHash(content: string): string {
  const hex = createHash("sha256").update(content).digest("hex");
  return `sha256:${hex}`;
}

function createUpgradeFixture(): string {
  const root = mkdtempSync(join(tmpdir(), "tanren-upgrade-bdd-"));

  const tanrenDir = join(root, ".tanren");
  const commandsDir = join(root, "commands");
  const standardsDir = join(root, "standards");
  mkdirSync(tanrenDir, { recursive: true });
  mkdirSync(commandsDir, { recursive: true });
  mkdirSync(standardsDir, { recursive: true });

  const configOld = "# Tanren configuration (old)\n";
  writeFileSync(join(tanrenDir, "config.toml"), configOld);

  const checkContent = "# Check command documentation\n";
  writeFileSync(join(commandsDir, "check.md"), checkContent);

  const retiredContent = "# Retired command documentation\n";
  writeFileSync(join(commandsDir, "retired.md"), retiredContent);

  const userContent = "# Team policy\n";
  writeFileSync(join(standardsDir, "team-policy.md"), userContent);

  const manifest = `version = 1
source_version = "${FIXTURE_SOURCE_VERSION}"

[[assets]]
path = ".tanren/config.toml"
hash = "sha256:0000000000000000abcdef0123456789"
ownership = "tanren"
installed_from = "${FIXTURE_SOURCE_VERSION}"

[[assets]]
path = "commands/check.md"
hash = "${computeHash(checkContent)}"
ownership = "tanren"
installed_from = "${FIXTURE_SOURCE_VERSION}"

[[assets]]
path = "commands/retired.md"
hash = "${computeHash(retiredContent)}"
ownership = "tanren"
installed_from = "${FIXTURE_SOURCE_VERSION}"

[[assets]]
path = "standards/team-policy.md"
hash = "${computeHash(userContent)}"
ownership = "user"
installed_from = "${FIXTURE_SOURCE_VERSION}"
`;

  writeFileSync(join(tanrenDir, "asset-manifest"), manifest);
  return root;
}

function apiUrl(): string {
  return process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
}

Given(
  /^a repository with Tanren assets installed at version "([^"]+)"$/,
  async () => {
    fixtureRoot = createUpgradeFixture();
    lastPreview = null;
  },
);

When("the user previews the upgrade", async () => {
  if (!fixtureRoot) throw new Error("fixture root not initialised");
  const res = await fetch(`${apiUrl()}/assets/upgrade/preview`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ root: fixtureRoot }),
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`upgrade preview failed: ${res.status} ${body}`);
  }
  lastPreview = (await res.json()) as UpgradePreviewData;
});

When("the user confirms and applies the upgrade", async () => {
  if (!fixtureRoot) throw new Error("fixture root not initialised");
  const res = await fetch(`${apiUrl()}/assets/upgrade/apply`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ root: fixtureRoot, confirm: true }),
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`upgrade apply failed: ${res.status} ${body}`);
  }
  lastPreview = (await res.json()) as UpgradePreviewData;
});

Then(
  "the preview includes actions to create, update, and remove generated assets",
  async () => {
    if (!lastPreview) throw new Error("no preview response");
    const kinds = lastPreview.actions.map((a) => a.action);
    if (!kinds.includes("create"))
      throw new Error("preview must include a create action");
    if (!kinds.includes("update"))
      throw new Error("preview must include an update action");
    if (!kinds.includes("remove"))
      throw new Error("preview must include a remove action");
  },
);

Then("the preview reports migration concerns", async () => {
  if (!lastPreview) throw new Error("no preview response");
  if (lastPreview.concerns.length === 0)
    throw new Error("preview must report at least one migration concern");
});

Then("the preview lists user-owned paths as preserved", async () => {
  if (!lastPreview) throw new Error("no preview response");
  if (lastPreview.preserved_user_paths.length === 0)
    throw new Error("preview must list preserved user paths");
});

Then("generated assets are updated to the target version", async () => {
  if (!fixtureRoot) throw new Error("fixture root not initialised");
  const configPath = join(fixtureRoot, ".tanren", "config.toml");
  const content = readFileSync(configPath, "utf-8");
  if (content !== "# Tanren configuration\n")
    throw new Error(
      `generated asset must reflect target version, got: ${JSON.stringify(content)}`,
    );
});

Then("user-owned files are unchanged", async () => {
  if (!fixtureRoot) throw new Error("fixture root not initialised");
  const userPath = join(fixtureRoot, "standards", "team-policy.md");
  const content = readFileSync(userPath, "utf-8");
  if (content !== "# Team policy\n")
    throw new Error(
      `user-owned file must be unchanged, got: ${JSON.stringify(content)}`,
    );
});

Then("the repository remains at the installed version", async () => {
  if (!fixtureRoot) throw new Error("fixture root not initialised");
  const configPath = join(fixtureRoot, ".tanren", "config.toml");
  const content = readFileSync(configPath, "utf-8");
  if (content !== "# Tanren configuration (old)\n")
    throw new Error(
      `generated asset must be unchanged after preview-only, got: ${JSON.stringify(content)}`,
    );
  const retiredPath = join(fixtureRoot, "commands", "retired.md");
  if (!existsSync(retiredPath))
    throw new Error("retired asset must still exist after preview-only");
});
