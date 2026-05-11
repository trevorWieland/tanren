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
  email: string | null;
  password: string | null;
}

interface RepositoryFixtureState {
  repository: string;
  fingerprint: string;
  priorCommits: number;
}

interface HostFixtureState {
  host: string;
}

interface WebProjectWorld {
  actors: Map<string, ProjectActorState>;
  repositories: Map<string, RepositoryFixtureState>;
  repositoryAccess: Map<string, Set<string>>;
  hosts: Map<string, HostFixtureState>;
  lastFailureCode: string | null;
  lastFailureSummary: string | null;
}

const extendedTest = accountTest.extend<{ projectWorld: WebProjectWorld }>({
  projectWorld: async ({}, use) => {
    await use({
      actors: new Map(),
      repositories: new Map(),
      repositoryAccess: new Map(),
      hosts: new Map(),
      lastFailureCode: null,
      lastFailureSummary: null,
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
      email: null,
      password: null,
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

function repositoryAccessSet(
  world: WebProjectWorld,
  actorName: string,
): Set<string> {
  let current = world.repositoryAccess.get(actorName);
  if (!current) {
    current = new Set<string>();
    world.repositoryAccess.set(actorName, current);
  }
  return current;
}

function setRepositoryAccess(
  world: WebProjectWorld,
  actorName: string,
  repository: string,
  allowed: boolean,
): void {
  const access = repositoryAccessSet(world, actorName);
  if (allowed) {
    access.add(repository);
  } else {
    access.delete(repository);
  }
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
  const raw = await response.text();
  const payload = raw.length > 0 ? (JSON.parse(raw) as T) : ({} as T);
  return {
    ok: response.ok(),
    status: response.status(),
    payload,
  };
}

async function createProjectAccount(
  page: Page,
  name: string,
): Promise<{ accountId: string; email: string }> {
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
  return { accountId: payload.account.id, email };
}

async function setRepositoryAccessFixture(
  page: Page,
  accountId: string,
  repository: string,
  allowed: boolean,
): Promise<void> {
  const response = await postWithSession<Record<string, never>>(
    page,
    "/test-hooks/source-control/repository-access",
    {
      account_id: accountId,
      repository,
      allowed,
    },
  );
  if (!response.ok) {
    throw new Error(
      `set repository access failed: ${response.status} ${repository} allowed=${allowed}`,
    );
  }
}

async function setHostCreateAccessFixture(
  page: Page,
  accountId: string,
  host: string,
  allowed: boolean,
): Promise<void> {
  const response = await postWithSession<Record<string, never>>(
    page,
    "/test-hooks/source-control/host-create-access",
    {
      account_id: accountId,
      host,
      allowed,
    },
  );
  if (!response.ok) {
    throw new Error(
      `set host create access failed: ${response.status} ${host} allowed=${allowed}`,
    );
  }
}

async function repositoryCreatedAtHostFixture(
  page: Page,
  repository: string,
  host: string,
): Promise<boolean> {
  const response = await postWithSession<{ created: boolean }>(
    page,
    "/test-hooks/source-control/repository-created",
    {
      host,
      repository,
    },
  );
  return response.payload.created === true;
}

Given(
  /^(\w+) has a project account$/,
  async ({ page, projectWorld }, name: string) => {
    const state = actor(projectWorld, name);
    await page.context().clearCookies();
    const { accountId, email } = await createProjectAccount(page, name);
    state.accountId = accountId;
    state.email = email;
    state.password = "fixture-password";
    state.connectedRepositories.clear();
    state.lastConnectedRepository = null;
    state.lastCreatedRepository = null;
    state.lastDesignatedHost = null;
    for (const repository of projectWorld.repositories.keys()) {
      setRepositoryAccess(projectWorld, name, repository, true);
      await setRepositoryAccessFixture(page, state.accountId, repository, true);
    }
    projectWorld.lastFailureCode = null;
    projectWorld.lastFailureSummary = null;
  },
);

Given(
  /^repository fixture "([^"]+)" has fingerprint "([^"]+)" and (\d+) prior commits$/,
  async (
    { page, projectWorld },
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
    for (const [actorName, state] of projectWorld.actors.entries()) {
      if (state.accountId) {
        await setRepositoryAccessFixture(
          page,
          state.accountId,
          canonical,
          true,
        );
        setRepositoryAccess(projectWorld, actorName, canonical, true);
      }
    }
  },
);

Given(
  /^repository fixture "([^"]+)" is accessible to (\w+)$/,
  async ({ page, projectWorld }, repository: string, name: string) => {
    const canonical = canonicalRepository(repository);
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account`);
    }
    setRepositoryAccess(projectWorld, name, canonical, true);
    await setRepositoryAccessFixture(page, state.accountId, canonical, true);
  },
);

Given(
  /^repository fixture "([^"]+)" is not accessible to (\w+)$/,
  async ({ page, projectWorld }, repository: string, name: string) => {
    const canonical = canonicalRepository(repository);
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account`);
    }
    setRepositoryAccess(projectWorld, name, canonical, false);
    await setRepositoryAccessFixture(page, state.accountId, canonical, false);
  },
);

Given(
  /^designated fixture host "([^"]+)" is accessible to (\w+)$/,
  async ({ page, projectWorld }, host: string, name: string) => {
    const canonical = canonicalHost(host);
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account id`);
    }
    projectWorld.hosts.set(canonical, { host: canonical });
    await setHostCreateAccessFixture(page, state.accountId, canonical, true);
  },
);

Given(
  /^designated fixture host "([^"]+)" is not accessible to (\w+)$/,
  async ({ page, projectWorld }, host: string, name: string) => {
    const canonical = canonicalHost(host);
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account id`);
    }
    projectWorld.hosts.set(canonical, { host: canonical });
    await setHostCreateAccessFixture(page, state.accountId, canonical, false);
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
      summary?: string;
      project?: { repository?: { repository?: string } };
    };

    if (response.ok()) {
      projectWorld.lastFailureCode = null;
      projectWorld.lastFailureSummary = null;
      state.lastConnectedRepository =
        payload.project?.repository?.repository ?? canonical;
      state.connectedRepositories.add(state.lastConnectedRepository);
      state.lastCreatedRepository = null;
      state.lastDesignatedHost = null;
      return;
    }

    projectWorld.lastFailureCode = payload.code ?? "unknown";
    projectWorld.lastFailureSummary = payload.summary ?? null;
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
    const payload = (await response.json()) as {
      code?: string;
      summary?: string;
    };

    projectWorld.lastFailureCode = payload.code ?? "unknown";
    projectWorld.lastFailureSummary = payload.summary ?? null;
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
      summary?: string;
      project?: { repository?: { repository?: string } };
    };

    state.lastDesignatedHost = canonicalHostName;
    if (response.ok()) {
      projectWorld.lastFailureCode = null;
      projectWorld.lastFailureSummary = null;
      state.lastConnectedRepository = null;
      state.lastCreatedRepository =
        payload.project?.repository?.repository ?? canonicalRepositoryName;
      state.connectedRepositories.add(state.lastCreatedRepository);
      return;
    }

    projectWorld.lastFailureCode = payload.code ?? "unknown";
    projectWorld.lastFailureSummary = payload.summary ?? null;
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
  /^the project failure summary is "([^"]+)"$/,
  async ({ projectWorld }, summary: string) => {
    if (projectWorld.lastFailureSummary !== summary) {
      throw new Error(
        `expected project failure summary "${summary}", got "${projectWorld.lastFailureSummary ?? "none"}"`,
      );
    }
  },
);

Then(
  /^source-control provider connect checks were called (\d+) times$/,
  async ({ page }, expectedRaw: string) => {
    const expected = Number.parseInt(expectedRaw, 10);
    const response = await postWithSession<{
      preflight_connect_repository: number;
      preflight_create_repository: number;
      create_repository: number;
      delete_repository: number;
    }>(page, "/test-hooks/source-control/call-counters", {});
    if (!response.ok) {
      throw new Error(
        `source-control call counters request failed: ${response.status}`,
      );
    }
    if (response.payload.preflight_connect_repository !== expected) {
      throw new Error(
        `expected preflight_connect_repository === ${expected}, got ${response.payload.preflight_connect_repository}`,
      );
    }
  },
);

Then(
  /^source-control provider create checks were called (\d+) times$/,
  async ({ page }, expectedRaw: string) => {
    const expected = Number.parseInt(expectedRaw, 10);
    const response = await postWithSession<{
      preflight_connect_repository: number;
      preflight_create_repository: number;
      create_repository: number;
      delete_repository: number;
    }>(page, "/test-hooks/source-control/call-counters", {});
    if (!response.ok) {
      throw new Error(
        `source-control call counters request failed: ${response.status}`,
      );
    }
    if (response.payload.preflight_create_repository !== expected) {
      throw new Error(
        `expected preflight_create_repository === ${expected}, got ${response.payload.preflight_create_repository}`,
      );
    }
    if (response.payload.create_repository !== expected) {
      throw new Error(
        `expected create_repository === ${expected}, got ${response.payload.create_repository}`,
      );
    }
  },
);

Given("the project store fails project registration", async ({ page }) => {
  const response = await postWithSession<Record<string, never>>(
    page,
    "/test-hooks/projects/break-store",
    {},
  );
  if (response.status !== 204) {
    throw new Error(`expected 204 from break-store, got ${response.status}`);
  }
});

When(
  /^(\w+) uses their credential to connect existing repository "([^"]+)" for (\w+) as an active project$/,
  async (
    { page, projectWorld },
    actorName: string,
    repository: string,
    targetActorName: string,
  ) => {
    const canonical = canonicalRepository(repository);
    const fixture = projectWorld.repositories.get(canonical);
    if (!fixture) {
      throw new Error(`repository fixture missing for ${canonical}`);
    }
    const sourceState = actor(projectWorld, actorName);
    if (!sourceState.accountId) {
      throw new Error(`actor ${actorName} has no project account`);
    }
    if (!sourceState.email || !sourceState.password) {
      throw new Error(`actor ${actorName} has no stored credentials`);
    }
    const targetState = actor(projectWorld, targetActorName);
    if (!targetState.accountId) {
      throw new Error(`actor ${targetActorName} has no project account`);
    }

    // Sign in as the source actor through the web form.
    await page.context().clearCookies();
    await page.goto("/sign-in");
    await page.getByLabel(/email/i).fill(sourceState.email);
    await page.getByLabel(/password/i).fill(sourceState.password);
    await page.getByRole("button", { name: /^sign in$/i }).click();
    await page.waitForURL("/", { timeout: 10_000 });

    const response = await postWithSession<{
      code?: string;
      summary?: string;
      project?: { repository?: { repository?: string } };
    }>(page, "/test-hooks/projects/connect-as-cross-account", {
      owning_account_id: targetState.accountId,
      repository,
      select_as_active: true,
    });

    if (response.ok) {
      projectWorld.lastFailureCode = null;
      projectWorld.lastFailureSummary = null;
      sourceState.lastConnectedRepository =
        response.payload.project?.repository?.repository ?? canonical;
      sourceState.connectedRepositories.add(
        sourceState.lastConnectedRepository,
      );
      sourceState.lastCreatedRepository = null;
      sourceState.lastDesignatedHost = null;
      return;
    }

    projectWorld.lastFailureCode = response.payload.code ?? "unknown";
    projectWorld.lastFailureSummary = response.payload.summary ?? null;
    sourceState.lastConnectedRepository = null;
    sourceState.lastCreatedRepository = null;
    sourceState.lastDesignatedHost = null;
  },
);

Then(
  /^repository "([^"]+)" exists at designated host "([^"]+)"$/,
  async (
    { page, projectWorld },
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
    const created = await repositoryCreatedAtHostFixture(
      page,
      canonicalRepositoryName,
      canonicalHostName,
    );
    if (!created) {
      throw new Error(
        `expected repository ${canonicalRepositoryName} to exist at designated host ${canonicalHostName}`,
      );
    }
  },
);

Then(
  /^repository "([^"]+)" does not exist at designated host "([^"]+)"$/,
  async (
    { page, projectWorld },
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
    const created = await repositoryCreatedAtHostFixture(
      page,
      canonicalRepositoryName,
      canonicalHostName,
    );
    if (created) {
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
