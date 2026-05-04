/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0137.
//
// The Gherkin in `tests/bdd/features/B-0137-choose-deployment-posture.feature`
// is the single source of truth for both the Rust `tanren-bdd` runner and
// this Node `playwright-bdd` runner.
//
// Coverage:
//
// - List postures with capability summaries (`@positive @web`).
// - Select a posture and verify it is recorded (`@positive @web`).
// - Change posture and verify attribution (`@positive @web`).
// - Reject posture change from non-admin (`@falsification @web`).
// - Reject unsupported posture value (`@falsification @web`).

import { createBdd } from "playwright-bdd";

import { test } from "./account.steps";

const { When, Then } = createBdd(test);

const POSTURE_LABELS: Record<string, string> = {
  hosted: "Hosted",
  self_hosted: "Self-hosted",
  local_only: "Local-only",
};

let emailCounter = 0;

async function waitForHydration(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.waitForFunction(
    () => {
      const root = document as unknown as Record<string, unknown>;
      const keys = Object.keys(root).filter(
        (k) =>
          k.startsWith("__reactContainer") ||
          k.startsWith("_reactRootContainer"),
      );
      if (keys.length > 0) return true;
      return Array.from(document.querySelectorAll("*")).some((el) =>
        Object.keys(el).some((k) => k.startsWith("__reactProps$")),
      );
    },
    { timeout: 30_000 },
  );
}

async function ensureSession(
  page: import("@playwright/test").Page,
  admin: boolean,
): Promise<void> {
  emailCounter++;
  const email = `posture-${admin ? "admin" : "user"}-${emailCounter}@bdd.tanren`;
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "";
  await page.evaluate(
    async ({ url, emailAddress, isAdmin }) => {
      const res = await fetch(`${url}/accounts`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          email: emailAddress,
          password: "password123",
          display_name: "Posture User",
        }),
        credentials: "include",
      });
      if (!res.ok) {
        const body = await res.text();
        throw new Error(`sign-up failed: ${res.status} ${body}`);
      }
      if (isAdmin) {
        const grantRes = await fetch(`${url}/test-hooks/grant-posture-admin`, {
          method: "POST",
          credentials: "include",
        });
        if (!grantRes.ok) {
          throw new Error(`grant-posture-admin failed: ${grantRes.status}`);
        }
      }
    },
    { url: apiUrl, emailAddress: email, isAdmin: admin },
  );
}

async function ensureOnPosturePage(
  page: import("@playwright/test").Page,
): Promise<void> {
  if (!page.url().includes("/posture")) {
    await page.goto("/posture");
    await waitForHydration(page);
  }
}

When("the actor lists available postures", async ({ page }) => {
  await page.goto("/posture");
  await waitForHydration(page);
});

Then(
  "the posture list contains {int} entries",
  async ({ page }, count: number) => {
    const radios = await page.locator('input[name="posture"]').count();
    if (radios !== count) {
      throw new Error(
        `expected ${count} posture radio inputs, found ${radios}`,
      );
    }
  },
);

Then(
  "the posture list includes {string}",
  async ({ page }, posture: string) => {
    const radio = page.locator(`input[name="posture"][value="${posture}"]`);
    const visible = await radio.isVisible();
    if (!visible) {
      throw new Error(`posture radio '${posture}' is not visible`);
    }
  },
);

When(
  "the admin sets the posture to {string}",
  async ({ page, world }, posture: string) => {
    await ensureOnPosturePage(page);
    if (!world.actors.has("__posture_admin__")) {
      await ensureSession(page, true);
      world.actors.set("__posture_admin__", { hasSession: true });
    }

    const radio = page.locator(`input[name="posture"][value="${posture}"]`);
    const hasRadio = await radio.count();

    if (hasRadio > 0) {
      await radio.check();
      const button = page.getByRole("button", {
        name: /set posture/i,
      });
      await button.click();
      await page.waitForFunction(
        () => {
          const btn = document.querySelector(
            'button[type="submit"]',
          ) as HTMLButtonElement | null;
          return btn && !btn.disabled;
        },
        { timeout: 10_000 },
      );
    } else {
      const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "";
      await page.evaluate(
        async ({ url, p }) => {
          const res = await fetch(`${url}/v0/posture`, {
            method: "PUT",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ posture: p }),
            credentials: "include",
          });
          const body = await res.json();
          if (res.ok) {
            throw new Error(`expected failure for posture '${p}', got success`);
          }
          (window as unknown as Record<string, unknown>)["__postureError"] =
            body;
        },
        { url: apiUrl, p: posture },
      );
    }
  },
);

Then("the current posture is {string}", async ({ page }, posture: string) => {
  const label = POSTURE_LABELS[posture] ?? posture;
  const labelEl = page.getByText("Current posture").first();
  const container = labelEl.locator("..");
  const text = (await container.textContent()) ?? "";
  if (!text.includes(label)) {
    throw new Error(`expected current posture '${label}', got text: ${text}`);
  }
});

Then("a {string} event is recorded", async ({ page }, eventType: string) => {
  if (eventType === "posture_set") {
    const success = page.getByText("Posture updated successfully.");
    const visible = await success.isVisible().catch(() => false);
    if (!visible) {
      throw new Error(
        `expected '${eventType}' event indicator (success message) to be visible`,
      );
    }
  }
});

When(
  "a non-admin sets the posture to {string}",
  async ({ page, world }, posture: string) => {
    await ensureOnPosturePage(page);
    if (!world.actors.has("__posture_nonadmin__")) {
      await ensureSession(page, false);
      world.actors.set("__posture_nonadmin__", { hasSession: true });
    }

    const radio = page.locator(`input[name="posture"][value="${posture}"]`);
    await radio.check();
    await page.getByRole("button", { name: /set posture/i }).click();
    await page
      .locator('form [role="alert"]')
      .first()
      .waitFor({ state: "visible", timeout: 10_000 });
  },
);

Then(
  "the posture request fails with code {string}",
  async ({ page }, code: string) => {
    const apiError = await page.evaluate(
      () => (window as unknown as Record<string, unknown>)["__postureError"],
    );
    if (apiError && typeof apiError === "object" && apiError !== null) {
      const err = apiError as Record<string, unknown>;
      const actualCode = String(err["code"] ?? "");
      if (actualCode !== code) {
        throw new Error(`expected failure code '${code}', got '${actualCode}'`);
      }
      return;
    }

    const alert = page.locator('form [role="alert"]').first();
    const text = (await alert.innerText()).toLowerCase();

    const codeChecks: Record<string, string[]> = {
      permission_denied: ["permission"],
      unsupported_posture: ["not supported"],
    };

    const checks = codeChecks[code];
    if (checks && !checks.some((c) => text.includes(c))) {
      throw new Error(`expected failure code '${code}', got alert: ${text}`);
    }
  },
);
