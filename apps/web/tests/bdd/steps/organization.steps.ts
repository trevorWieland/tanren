/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0066.
//
// Organization-creation positive and falsification scenarios exercised
// through the Next.js frontend against the live api server.

import { expect } from "@playwright/test";
import { createBdd } from "playwright-bdd";
import { test, actor } from "./account.steps";

const { Given, When, Then } = createBdd(test);

async function waitForHydration(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.waitForFunction(
    () =>
      Array.from(document.querySelectorAll("*")).some((el) =>
        Object.keys(el).some((k) => k.startsWith("__reactProps$")),
      ),
    { timeout: 30_000 },
  );
}

When(
  /^(\w+) creates an organization named "([^"]+)"$/,
  async ({ page }, _actorName: string, name: string) => {
    await page.goto("/organizations/new");
    await waitForHydration(page);
    await page.getByLabel(/organization name/i).fill(name);
    await page.getByRole("button", { name: /create organization/i }).click();
    await expect(
      page.locator('[data-testid="created-organization"]'),
    ).toBeVisible({ timeout: 10_000 });
  },
);

Given(
  /^(\w+) has created an organization named "([^"]+)"$/,
  async ({ page, world }, actorName: string, name: string) => {
    const a = world.actors.get(actorName);
    if (!a?.hasSession) {
      throw new Error(`${actorName} must be signed in to create an org`);
    }
    await page.goto("/organizations/new");
    await waitForHydration(page);
    await page.getByLabel(/organization name/i).fill(name);
    await page.getByRole("button", { name: /create organization/i }).click();
    await expect(
      page.locator('[data-testid="created-organization"]'),
    ).toBeVisible({ timeout: 10_000 });
  },
);

Then("the response includes full bootstrap permissions", async ({ page }) => {
  const container = page.locator('[data-testid="membership-permissions"]');
  await expect(container).toBeVisible({ timeout: 5_000 });
  for (const flag of [
    "invite",
    "manage_access",
    "configure",
    "set_policy",
    "delete",
  ]) {
    await expect(
      container.locator(`[data-testid="permission-${flag}"]`),
    ).toBeVisible({ timeout: 2_000 });
  }
});

Then(
  /^"([^"]+)" appears in (\w+)'s organization list$/,
  async ({ page }, orgName: string, _actorName: string) => {
    await page.goto("/organizations");
    await waitForHydration(page);
    await expect(page.getByText(new RegExp(orgName, "i")).first()).toBeVisible({
      timeout: 5_000,
    });
  },
);

When(
  /^an unsigned-in attempt creates an organization named "([^"]+)"$/,
  async ({ page, world }, _name: string) => {
    const a = actor(world, "anonymous");
    await page.goto("/organizations/new");
    await page.waitForURL(/\/sign-in/, { timeout: 10_000 });
    a.hasSession = false;
    a.lastFailureCode = "unauthenticated";
  },
);

Then(
  /^(\w+)'s admin permissions on "([^"]+)" are empty$/,
  async ({ page }, _actorName: string, orgName: string) => {
    await page.goto("/organizations");
    await waitForHydration(page);
    const items = page.getByRole("listitem");
    const count = await items.count();
    if (count === 0) return;
    const matched = items.filter({ hasText: new RegExp(orgName, "i") });
    const matchCount = await matched.count();
    if (matchCount === 0) return;
    const orgText = await matched.first().innerText();
    const hasPermissionMention = /invite|manage|configure|delete|admin/i.test(
      orgText,
    );
    if (hasPermissionMention) {
      throw new Error(
        `expected no admin permissions for ${orgName}, but found permission flags`,
      );
    }
  },
);
