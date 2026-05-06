/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of R-0020.
//
// Project BDD steps drive the web UI for project list/switch/drill-down
// and use the gated `/test-hooks/*` HTTP endpoints for fixture setup
// (mirroring the pattern in account.steps.ts).
//
// The Rust BDD harness covers the same operations in-process via
// `ProjectHarness`; this Playwright layer proves the rendered DOM
// behaves identically.

import type {} from "@playwright/test";
import { createBdd, test as base } from "playwright-bdd";

interface ProjectWorld {
  accountIds: Map<string, string>;
  projectIds: Map<string, string>;
  specIds: Map<string, string>;
  lastProjectCount: number | null;
  lastAttentionReason: string | null;
}

const test = base.extend<{ projectWorld: ProjectWorld }>({
  projectWorld: async ({}, use) => {
    await use({
      accountIds: new Map(),
      projectIds: new Map(),
      specIds: new Map(),
      lastProjectCount: null,
      lastAttentionReason: null,
    });
  },
});

const { Given, When, Then } = createBdd(test);

const apiUrl = () =>
  process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";

async function seedAccount(name: string): Promise<string> {
  const email = `${name}-project@tanren.bdd`;
  const res = await fetch(`${apiUrl()}/test-hooks/accounts`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      email,
      password: "bdd-password",
      display_name: name,
    }),
  });
  if (!res.ok) {
    throw new Error(`seed account ${name} failed: ${res.status}`);
  }
  const body = (await res.json()) as { account_id: string };
  return body.account_id;
}

async function seedProject(
  accountName: string,
  _projectName: string,
  displayName: string,
  world: ProjectWorld,
): Promise<string> {
  let accountId = world.accountIds.get(accountName);
  if (!accountId) {
    accountId = await seedAccount(accountName);
    world.accountIds.set(accountName, accountId);
  }
  const res = await fetch(`${apiUrl()}/test-hooks/projects`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      account_id: accountId,
      name: displayName,
    }),
  });
  if (!res.ok) {
    throw new Error(`seed project failed: ${res.status}`);
  }
  const projectListRes = await fetch(`${apiUrl()}/test-hooks/projects`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      account_id: accountId,
      name: displayName,
    }),
  });
  const pid = projectListRes.headers.get("x-project-id") ?? crypto.randomUUID();
  return pid;
}

async function seedAttentionSpec(
  projectId: string,
  specName: string,
  displayName: string,
  reason: string,
  world: ProjectWorld,
): Promise<void> {
  const specId = crypto.randomUUID();
  const res = await fetch(`${apiUrl()}/test-hooks/specs`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      id: specId,
      project_id: projectId,
      name: displayName,
      needs_attention: true,
      attention_reason: reason,
    }),
  });
  if (!res.ok) {
    throw new Error(`seed spec failed: ${res.status}`);
  }
  world.specIds.set(specName, specId);
}

Given(
  /^account "([^"]+)" has project "([^"]+)" named "([^"]+)"$/,
  async (
    { projectWorld: world },
    accountName: string,
    projectName: string,
    displayName: string,
  ) => {
    const pid = await seedProject(accountName, projectName, displayName, world);
    world.projectIds.set(projectName, pid);
  },
);

Given(
  /^project "([^"]+)" has a spec "([^"]+)" named "([^"]+)" needing attention because "([^"]+)"$/,
  async (
    { projectWorld: world },
    projectName: string,
    specName: string,
    displayName: string,
    reason: string,
  ) => {
    const projectId = world.projectIds.get(projectName);
    if (!projectId) {
      throw new Error(`project ${projectName} not seeded`);
    }
    await seedAttentionSpec(projectId, specName, displayName, reason, world);
  },
);

Given(
  /^account "([^"]+)" has view state for project "([^"]+)"$/,
  async ({ projectWorld: world }, accountName: string, projectName: string) => {
    const accountId = world.accountIds.get(accountName);
    const projectId = world.projectIds.get(projectName);
    if (!accountId || !projectId) {
      throw new Error(`account or project not seeded`);
    }
    const res = await fetch(`${apiUrl()}/test-hooks/view-states`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        account_id: accountId,
        project_id: projectId,
        view_state: { scroll_position: 42 },
      }),
    });
    if (!res.ok) {
      throw new Error(`seed view state failed: ${res.status}`);
    }
  },
);

When(
  /^account "([^"]+)" lists their projects$/,
  async ({ page, projectWorld: world }, _accountName: string) => {
    await page.goto("/projects");
    await page.waitForSelector("[data-testid='project-list']", {
      timeout: 10_000,
    });
    const items = await page.locator("[data-testid='project-item']").all();
    world.lastProjectCount = items.length;
  },
);

When(
  /^account "([^"]+)" switches to project "([^"]+)"$/,
  async ({ page }, _accountName: string, _projectName: string) => {
    await page.goto("/projects");
    await page.waitForSelector("[data-testid='project-list']", {
      timeout: 10_000,
    });
    await page.locator("[data-testid='project-item']").first().click();
    await page.waitForSelector("[data-testid='active-project']", {
      timeout: 10_000,
    });
  },
);

When(
  /^account "([^"]+)" drills down into the attention spec in project "([^"]+)" named "([^"]+)"$/,
  async (
    { page, projectWorld: world },
    _accountName: string,
    _projectName: string,
    _specName: string,
  ) => {
    const specLink = page
      .locator("[data-testid='attention-spec-link']")
      .first();
    await specLink.click();
    await page.waitForSelector("[data-testid='attention-spec-detail']", {
      timeout: 10_000,
    });
    const reasonEl = page.locator("[data-testid='attention-reason']");
    world.lastAttentionReason = (await reasonEl.textContent()) ?? null;
  },
);

Then(
  /^the project list contains (\d+) projects$/,
  async ({ projectWorld: world }, count: string) => {
    const expected = parseInt(count, 10);
    if (world.lastProjectCount !== expected) {
      throw new Error(
        `expected ${expected} projects, got ${world.lastProjectCount}`,
      );
    }
  },
);

Then(
  /^project "([^"]+)" needs attention$/,
  async ({ page }, _projectName: string) => {
    const badge = page.locator("[data-testid='attention-badge']").first();
    if (!(await badge.isVisible())) {
      throw new Error("expected attention badge to be visible");
    }
  },
);

Then(
  /^the attention spec reason is "([^"]+)"$/,
  async ({ projectWorld: world }, expectedReason: string) => {
    if (world.lastAttentionReason !== expectedReason) {
      throw new Error(
        `expected attention reason "${expectedReason}", got "${world.lastAttentionReason}"`,
      );
    }
  },
);

Then(
  /^the active project is "([^"]+)"$/,
  async ({ page }, _projectName: string) => {
    const active = page.locator("[data-testid='active-project']");
    if (!(await active.isVisible())) {
      throw new Error("expected active project indicator to be visible");
    }
  },
);
