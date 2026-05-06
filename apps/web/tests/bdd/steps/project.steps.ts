/* eslint-disable */

import { createBdd, test as base } from "playwright-bdd";

import type { ProjectView } from "@/app/lib/project-client";

interface ProjectActorState {
  lastProject?: ProjectView;
  lastFailureCode?: string | undefined;
}

interface ProjectWorld {
  actors: Map<string, ProjectActorState>;
}

export const test = base.extend<{ projectWorld: ProjectWorld }>({
  projectWorld: async ({}, use) => {
    await use({ actors: new Map() });
  },
});

const { When, Then } = createBdd(test);

function actor(world: ProjectWorld, name: string): ProjectActorState {
  let state = world.actors.get(name);
  if (!state) {
    state = {};
    world.actors.set(name, state);
  }
  return state;
}

When(
  /^(\w+) connects repository "([^"]+)" as project "([^"]+)"$/,
  async (
    { page, projectWorld },
    name: string,
    repoUrl: string,
    projectName: string,
  ) => {
    const a = actor(projectWorld, name);
    await page.goto("/projects/new");
    await waitForHydration(page);
    await page
      .getByLabel(/project name/i)
      .first()
      .fill(projectName);
    await page.getByLabel(/repository url/i).fill(repoUrl);
    await page.getByRole("button", { name: /connect repository/i }).click();
    const result = await Promise.race([
      page
        .getByRole("heading", { name: /active project/i })
        .waitFor({ state: "visible" })
        .then(() => "ok" as const),
      page
        .locator('form [role="alert"]')
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.lastFailureCode = undefined;
    } else {
      a.lastFailureCode = await classifyProjectFailureFromAlert(page);
    }
  },
);

When(
  /^(\w+) creates a new project "([^"]+)" on provider "([^"]+)"$/,
  async (
    { page, projectWorld },
    name: string,
    projectName: string,
    providerHost: string,
  ) => {
    const a = actor(projectWorld, name);
    await page.goto("/projects/new");
    await waitForHydration(page);
    await page
      .getByRole("button", { name: /create a new repository/i })
      .click();
    await page
      .getByLabel(/project name/i)
      .first()
      .fill(projectName);
    await page.getByLabel(/provider host/i).fill(providerHost);
    await page.getByRole("button", { name: /^create project$/i }).click();
    const result = await Promise.race([
      page
        .getByRole("heading", { name: /active project/i })
        .waitFor({ state: "visible" })
        .then(() => "ok" as const),
      page
        .locator('form [role="alert"]')
        .first()
        .waitFor({ state: "visible" })
        .then(() => "alert" as const),
    ]);
    if (result === "ok") {
      a.lastFailureCode = undefined;
    } else {
      a.lastFailureCode = await classifyProjectFailureFromAlert(page);
    }
  },
);

Then(
  /^the project request fails with code "([^"]+)"$/,
  async ({ projectWorld }, code: string) => {
    const failing = [...projectWorld.actors.values()].find(
      (a) => a.lastFailureCode !== undefined,
    );
    if (!failing) {
      throw new Error("expected at least one actor to have a project failure");
    }
    const observed = failing.lastFailureCode ?? "unknown";
    if (observed !== code) {
      throw new Error(`expected project failure code ${code}, got ${observed}`);
    }
  },
);

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

async function classifyProjectFailureFromAlert(
  page: import("@playwright/test").Page,
): Promise<string> {
  const text = await page.locator('form [role="alert"]').first().innerText();
  const colonIndex = text.indexOf(":");
  if (colonIndex > 0) {
    const candidate = text.slice(0, colonIndex).trim();
    if (/^[a-z_]+$/.test(candidate)) {
      return candidate;
    }
  }
  return "unknown";
}
