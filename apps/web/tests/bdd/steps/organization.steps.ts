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
  async ({ page, world }, name: string) => {
    const a = actor(world, "anonymous");
    await page.goto("/organizations/new");
    await page.waitForURL(/\/sign-in/, { timeout: 10_000 });
    a.hasSession = false;

    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";
    const res = await fetch(`${apiUrl}/organizations`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name }),
    });

    if (res.ok) {
      const body = await res.text();
      throw new Error(
        `unauthenticated POST /organizations unexpectedly succeeded (${res.status}): ${body}`,
      );
    }

    a.lastFailureCode = mapHttpStatusToFailureCode(res.status);
  },
);

Then(
  /^(\w+)'s admin permissions on "([^"]+)" are empty$/,
  async ({ page }, _actorName: string, orgName: string) => {
    const apiUrl =
      process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";
    const cookies = await page.context().cookies();
    const cookieHeader = cookies.map((c) => `${c.name}=${c.value}`).join("; ");

    const res = await fetch(`${apiUrl}/organizations`, {
      method: "GET",
      headers: { cookie: cookieHeader },
    });

    if (!res.ok) {
      const body = await res.text();
      throw new Error(
        `GET ${apiUrl}/organizations returned ${res.status}: ${body}`,
      );
    }

    const data = (await res.json()) as {
      organizations: Array<{
        id: string;
        name: string;
        created_at: string;
      }>;
    };

    const found = data.organizations.find(
      (org) => org.name.toLowerCase() === orgName.toLowerCase(),
    );

    if (found) {
      throw new Error(
        `expected "${orgName}" to be absent from the actor's organization list, but found it (id=${found.id})`,
      );
    }
  },
);

function mapHttpStatusToFailureCode(status: number): string {
  if (status === 401) return "unauthenticated";
  if (status === 403) return "forbidden";
  if (status === 422) return "validation_failed";
  return `http_${status}`;
}
