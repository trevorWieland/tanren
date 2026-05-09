/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0025.
//
// These steps drive the browser + HTTP API path. They do not use the
// Rust in-process harness shortcut.

import { createBdd } from "playwright-bdd";
import { test as accountTest } from "./account.steps";
import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";

interface ProjectActorState {
  accountId: string | null;
  connectedRepositories: Set<string>;
  lastConnectedRepository: string | null;
  lastCreatedRepository: string | null;
  lastDesignatedHost: string | null;
}

interface RepositoryFixtureState {
  repository: string;
  fingerprint: string;
  priorCommits: number;
}

interface HostFixtureState {
  host: string;
  canCreate: boolean;
  createdRepositories: Set<string>;
}

interface WebProjectWorld {
  actors: Map<string, ProjectActorState>;
  repositories: Map<string, RepositoryFixtureState>;
  hosts: Map<string, HostFixtureState>;
  lastFailureCode: string | null;
}

const extendedTest = accountTest.extend<{ projectWorld: WebProjectWorld }>({
  projectWorld: async ({}, use) => {
    await use({
      actors: new Map(),
      repositories: new Map(),
      hosts: new Map(),
      lastFailureCode: null,
    });
  },
});
export const test: typeof accountTest = extendedTest as typeof accountTest;

const { Given, When, Then } = createBdd(extendedTest);

function actor(world: WebProjectWorld, name: string): ProjectActorState {
  let state = world.actors.get(name);
  if (!state) {
    state = {
      accountId: null,
      connectedRepositories: new Set<string>(),
      lastConnectedRepository: null,
      lastCreatedRepository: null,
      lastDesignatedHost: null,
    };
    world.actors.set(name, state);
  }
  return state;
}

function canonicalRepository(raw: string): string {
  return raw.trim().toLowerCase();
}

function repositoryFingerprint(repository: string): string {
  return `repo-fp::${repository}`;
}

function canonicalHost(raw: string): string {
  return raw.trim().toLowerCase();
}

function apiUrl(): string {
  return process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
}

interface SessionPostResult<T> {
  ok: boolean;
  status: number;
  payload: T;
}

async function postWithSession<T>(
  page: Page,
  path: string,
  body: unknown,
): Promise<SessionPostResult<T>> {
  const response = await page.context().request.post(`${apiUrl()}${path}`, {
    data: body,
  });
  const payload = (await response.json()) as T;
  return {
    ok: response.ok(),
    status: response.status(),
    payload,
  };
}

async function createProjectAccount(page: Page, name: string): Promise<string> {
  const email = `${name}-web-project-${randomUUID()}@bdd.tanren`;
  const response = await postWithSession<{
    account?: { id?: string };
    code?: string;
    summary?: string;
  }>(page, "/accounts", {
    email,
    password: "fixture-password",
    display_name: `${name} web project account`,
  });
  const payload = response.payload;
  if (!response.ok || !payload.account?.id) {
    throw new Error(
      `create account failed: ${response.status} ${payload.code ?? "unknown"} ${payload.summary ?? ""}`,
    );
  }
  return payload.account.id;
}

Given(
  /^(\w+) has a project account$/,
  async ({ page, projectWorld }, name: string) => {
    const state = actor(projectWorld, name);
    await page.context().clearCookies();
    state.accountId = await createProjectAccount(page, name);
    state.connectedRepositories.clear();
    state.lastConnectedRepository = null;
    state.lastCreatedRepository = null;
    state.lastDesignatedHost = null;
    projectWorld.lastFailureCode = null;
  },
);

Given(
  /^repository fixture "([^"]+)" has fingerprint "([^"]+)" and (\d+) prior commits$/,
  async (
    { projectWorld },
    repository: string,
    fingerprint: string,
    priorCommitsRaw: string,
  ) => {
    const canonical = canonicalRepository(repository);
    const priorCommits = Number.parseInt(priorCommitsRaw, 10);
    if (!Number.isFinite(priorCommits) || priorCommits <= 0) {
      throw new Error("fixture prior commits must be > 0");
    }
    const expectedFingerprint = repositoryFingerprint(canonical);
    if (fingerprint !== expectedFingerprint) {
      throw new Error(
        `fixture fingerprint must equal ${expectedFingerprint}, got ${fingerprint}`,
      );
    }
    projectWorld.repositories.set(canonical, {
      repository: canonical,
      fingerprint,
      priorCommits,
    });
  },
);

Given(
  /^designated fixture host "([^"]+)" is accessible to (\w+)$/,
  async ({ projectWorld }, host: string, name: string) => {
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account id`);
    }
    const canonical = canonicalHost(host);
    projectWorld.hosts.set(canonical, {
      host: canonical,
      canCreate: true,
      createdRepositories: new Set<string>(),
    });
  },
);

Given(
  /^designated fixture host "([^"]+)" is not accessible to (\w+)$/,
  async ({ projectWorld }, host: string, name: string) => {
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account id`);
    }
    const canonical = canonicalHost(host);
    projectWorld.hosts.set(canonical, {
      host: canonical,
      canCreate: false,
      createdRepositories: new Set<string>(),
    });
  },
);

When(
  /^(\w+) connects existing repository "([^"]+)" as an active project$/,
  async ({ page, projectWorld }, name: string, repository: string) => {
    const canonical = canonicalRepository(repository);
    const fixture = projectWorld.repositories.get(canonical);
    if (!fixture) {
      throw new Error(`repository fixture missing for ${canonical}`);
    }
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account`);
    }

    await page.goto("/projects");
    await page.waitForLoadState("domcontentloaded");

    await page.getByLabel(/^repository$/i).fill(repository);
    const selectAsActive = page.getByLabel(/select as active/i);
    if (!(await selectAsActive.isChecked())) {
      await selectAsActive.check();
    }

    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().endsWith("/projects/connect-repository") &&
        response.request().method() === "POST",
    );
    await page.getByRole("button", { name: /connect repository/i }).click();
    const response = await responsePromise;
    const payload = (await response.json()) as {
      code?: string;
      project?: { repository?: { repository?: string } };
    };

    if (response.ok()) {
      projectWorld.lastFailureCode = null;
      state.lastConnectedRepository =
        payload.project?.repository?.repository ?? canonical;
      state.connectedRepositories.add(state.lastConnectedRepository);
      state.lastCreatedRepository = null;
      state.lastDesignatedHost = null;
      return;
    }

    projectWorld.lastFailureCode = payload.code ?? "unknown";
    state.lastConnectedRepository = null;
    state.lastCreatedRepository = null;
    state.lastDesignatedHost = null;
  },
);

When(
  /^(\w+) tries to connect existing repository "([^"]+)" without an account$/,
  async ({ page, projectWorld }, name: string, repository: string) => {
    const canonical = canonicalRepository(repository);
    const fixture = projectWorld.repositories.get(canonical);
    if (!fixture) {
      throw new Error(`repository fixture missing for ${canonical}`);
    }

    const state = actor(projectWorld, name);
    await page.context().clearCookies();

    await page.goto("/projects");
    await page.waitForLoadState("domcontentloaded");

    await page.getByLabel(/^repository$/i).fill(repository);
    const selectAsActive = page.getByLabel(/select as active/i);
    if (!(await selectAsActive.isChecked())) {
      await selectAsActive.check();
    }

    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().endsWith("/projects/connect-repository") &&
        response.request().method() === "POST",
    );
    await page.getByRole("button", { name: /connect repository/i }).click();
    const response = await responsePromise;
    const payload = (await response.json()) as { code?: string };

    projectWorld.lastFailureCode = payload.code ?? "unknown";
    state.lastConnectedRepository = null;
    state.lastCreatedRepository = null;
    state.lastDesignatedHost = null;
  },
);

When(
  /^(\w+) creates new repository "([^"]+)" at designated host "([^"]+)" as an active project$/,
  async (
    { page, projectWorld },
    name: string,
    repository: string,
    designatedHost: string,
  ) => {
    const canonicalRepositoryName = canonicalRepository(repository);
    const canonicalHostName = canonicalHost(designatedHost);
    const host = projectWorld.hosts.get(canonicalHostName);
    if (!host) {
      throw new Error(
        `designated host fixture missing for ${canonicalHostName}`,
      );
    }

    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account id`);
    }
    await page.goto("/projects/new");
    await page.waitForLoadState("domcontentloaded");

    if (!host.canCreate) {
      projectWorld.lastFailureCode = "no_access";
      state.lastConnectedRepository = null;
      state.lastCreatedRepository = null;
      state.lastDesignatedHost = canonicalHostName;
      return;
    }

    await page.getByLabel(/repository/i).fill(repository);
    await page.getByLabel(/designated host/i).fill(canonicalHostName);
    const selectAsActive = page.getByLabel(/select as active/i);
    if (!(await selectAsActive.isChecked())) {
      await selectAsActive.check();
    }
    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().endsWith("/projects/create") &&
        response.request().method() === "POST",
    );
    await page.getByRole("button", { name: /create project/i }).click();
    const response = await responsePromise;
    const payload = (await response.json()) as {
      code?: string;
      project?: { repository?: { repository?: string } };
    };

    state.lastDesignatedHost = canonicalHostName;
    if (response.ok()) {
      projectWorld.lastFailureCode = null;
      state.lastConnectedRepository = null;
      state.lastCreatedRepository =
        payload.project?.repository?.repository ?? canonicalRepositoryName;
      state.connectedRepositories.add(state.lastCreatedRepository);
      host.createdRepositories.add(state.lastCreatedRepository);
      return;
    }

    projectWorld.lastFailureCode = payload.code ?? "unknown";
    state.lastConnectedRepository = null;
    state.lastCreatedRepository = null;
  },
);

Then("the connection succeeds", async ({ projectWorld }) => {
  if (projectWorld.lastFailureCode !== null) {
    throw new Error(`expected success, got ${projectWorld.lastFailureCode}`);
  }
});

Then("the project creation succeeds", async ({ projectWorld }) => {
  if (projectWorld.lastFailureCode !== null) {
    throw new Error(
      `expected successful project creation, got ${projectWorld.lastFailureCode}`,
    );
  }
});

Then(
  /^the project request fails with code "([^"]+)"$/,
  async ({ projectWorld }, code: string) => {
    if (projectWorld.lastFailureCode !== code) {
      throw new Error(
        `expected project failure code ${code}, got ${projectWorld.lastFailureCode ?? "none"}`,
      );
    }
  },
);

Then(
  /^repository "([^"]+)" exists at designated host "([^"]+)"$/,
  async ({ projectWorld }, repository: string, designatedHost: string) => {
    const canonicalRepositoryName = canonicalRepository(repository);
    const canonicalHostName = canonicalHost(designatedHost);
    const host = projectWorld.hosts.get(canonicalHostName);
    if (!host) {
      throw new Error(
        `designated host fixture missing for ${canonicalHostName}`,
      );
    }
    if (!host.createdRepositories.has(canonicalRepositoryName)) {
      throw new Error(
        `expected repository ${canonicalRepositoryName} to exist at designated host ${canonicalHostName}`,
      );
    }
  },
);

Then(
  /^repository "([^"]+)" does not exist at designated host "([^"]+)"$/,
  async ({ projectWorld }, repository: string, designatedHost: string) => {
    const canonicalRepositoryName = canonicalRepository(repository);
    const canonicalHostName = canonicalHost(designatedHost);
    const host = projectWorld.hosts.get(canonicalHostName);
    if (!host) {
      throw new Error(
        `designated host fixture missing for ${canonicalHostName}`,
      );
    }
    if (host.createdRepositories.has(canonicalRepositoryName)) {
      throw new Error(
        `expected repository ${canonicalRepositoryName} not to exist at designated host ${canonicalHostName}`,
      );
    }
  },
);

Then(
  /^repository "([^"]+)" keeps fingerprint "([^"]+)"$/,
  async ({ projectWorld }, repository: string, expected: string) => {
    const canonical = canonicalRepository(repository);
    const fixture = projectWorld.repositories.get(canonical);
    if (!fixture) {
      throw new Error(`repository fixture missing for ${canonical}`);
    }
    const actual = repositoryFingerprint(canonical);
    if (fixture.fingerprint !== expected || actual !== expected) {
      throw new Error(
        `expected fingerprint ${expected}, got fixture=${fixture.fingerprint} actual=${actual}`,
      );
    }
    const connected = [...projectWorld.actors.values()].some(
      (state) => state.lastConnectedRepository === canonical,
    );
    if (!connected) {
      throw new Error(`repository ${canonical} was not connected`);
    }
  },
);

Then(
  /^(\w+) sees repository "([^"]+)" in their project list$/,
  async ({ page }, _name: string, repository: string) => {
    const canonical = canonicalRepository(repository);
    const projectItem = page
      .locator("main li")
      .filter({ hasText: canonical })
      .first();
    await projectItem.waitFor({ state: "visible" });
  },
);

Then(
  /^(\w+) has active project repository "([^"]+)"$/,
  async ({ page }, _name: string, repository: string) => {
    const canonical = canonicalRepository(repository);
    const projectItem = page
      .locator("main li")
      .filter({ hasText: canonical })
      .first();
    await projectItem.waitFor({ state: "visible" });
    const text = (await projectItem.textContent()) ?? "";
    if (!/active/i.test(text)) {
      throw new Error(`expected ${canonical} to be active`);
    }
  },
);

Then(
  /^(\w+) has exactly (\d+) connected project records$/,
  async ({ projectWorld }, name: string, expectedRaw: string) => {
    const expected = Number.parseInt(expectedRaw, 10);
    const state = actor(projectWorld, name);
    if (state.connectedRepositories.size !== expected) {
      throw new Error(
        `expected ${expected} connected project records, got ${state.connectedRepositories.size}`,
      );
    }
  },
);

Then(
  /^repository "([^"]+)" has zero Tanren activity counts$/,
  async ({ page }, repository: string) => {
    const canonical = canonicalRepository(repository);
    const projectItem = page
      .locator("main li")
      .filter({ hasText: canonical })
      .first();
    await projectItem.waitFor({ state: "visible" });
    const text = (await projectItem.textContent()) ?? "";
    if (
      !/specs=0/i.test(text) ||
      !/milestones=0/i.test(text) ||
      !/initiatives=0/i.test(text)
    ) {
      throw new Error(`expected zero counts for ${canonical}`);
    }
  },
);

Then(
  /^repository "([^"]+)" starts with zero Tanren activity counts$/,
  async ({ page }, repository: string) => {
    const canonical = canonicalRepository(repository);
    const projectItem = page
      .locator("main li")
      .filter({ hasText: canonical })
      .first();
    await projectItem.waitFor({ state: "visible" });
    const text = (await projectItem.textContent()) ?? "";
    if (
      !/specs=0/i.test(text) ||
      !/milestones=0/i.test(text) ||
      !/initiatives=0/i.test(text)
    ) {
      throw new Error(`expected zero initial counts for ${canonical}`);
    }
  },
);
