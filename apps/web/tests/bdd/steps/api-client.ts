import type { Page } from "@playwright/test";

import { actor, type OrganizationWorld } from "./organization-world";

export async function signInActorViaUi(
  page: Page,
  world: OrganizationWorld,
  name: string,
): Promise<void> {
  const a = actor(world, name);
  if (!a.email || !a.password) {
    throw new Error(`actor ${name} has no recorded credentials`);
  }

  await page.context().clearCookies();
  await page.goto("/sign-in");
  await waitForHydration(page);
  await page.getByLabel(/email/i).fill(a.email);
  await page.getByLabel(/password/i).fill(a.password);
  await page.getByRole("button", { name: /^sign in$/i }).click();

  const result = await Promise.race([
    page.waitForURL("/").then(() => "ok" as const),
    page
      .locator('form [role="alert"]')
      .first()
      .waitFor({ state: "visible" })
      .then(() => "alert" as const),
  ]);

  if (result !== "ok") {
    a.hasSession = false;
    a.lastFailureCode = await classifyFailureFromAlert(page);
    throw new Error(`sign-in failed for ${name} via web UI`);
  }

  a.hasSession = true;
  delete a.lastFailureCode;
}

export async function waitForHydration(page: Page): Promise<void> {
  await page.waitForFunction(
    () => {
      const root = document as unknown as Record<string, unknown>;
      const keys = Object.keys(root).filter(
        (key) =>
          key.startsWith("__reactContainer") ||
          key.startsWith("_reactRootContainer"),
      );
      if (keys.length > 0) {
        return true;
      }

      return Array.from(document.querySelectorAll("*")).some((el) =>
        Object.keys(el).some((key) => key.startsWith("__reactProps$")),
      );
    },
    { timeout: 30_000 },
  );
}

async function classifyFailureFromAlert(page: Page): Promise<string> {
  const text = (
    await page.locator('form [role="alert"]').first().innerText()
  ).toLowerCase();

  if (text.includes("invitation") && text.includes("expired")) {
    return "invitation_expired";
  }
  if (text.includes("invitation") && text.includes("not recognized")) {
    return "invitation_not_found";
  }
  if (text.includes("invitation") && text.includes("already been accepted")) {
    return "invitation_already_consumed";
  }
  if (text.includes("account already exists")) {
    return "duplicate_identifier";
  }
  if (text.includes("email or password is invalid")) {
    return "invalid_credential";
  }
  if (text.includes("check the form fields") || text.includes("required")) {
    return "validation_failed";
  }

  return "unknown";
}
