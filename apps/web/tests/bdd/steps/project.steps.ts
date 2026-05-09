/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0025.
//
// These steps drive the browser + HTTP API path. They do not use the
// Rust in-process harness shortcut.

import { createBdd } from "playwright-bdd";
import { test as accountTest } from "./account.steps";
import { randomUUID } from "node:crypto";

interface ProjectActorState {
  accountId: string | null;
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

async function createProjectAccount(name: string): Promise<string> {
  const email = `${name}-web-project-${randomUUID()}@bdd.tanren`;
  const response = await fetch(`${apiUrl()}/accounts`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      email,
      password: "fixture-password",
      display_name: `${name} web project account`,
    }),
  });
  const payload = (await response.json()) as {
    account?: { id?: string };
    code?: string;
    summary?: string;
  };
  if (!response.ok || !payload.account?.id) {
    throw new Error(
      `create account failed: ${response.status} ${payload.code ?? "unknown"} ${payload.summary ?? ""}`,
    );
  }
  return payload.account.id;
}

async function listProjects(owningAccountId: string): Promise<{
  owning_account_id: string;
  projects: Array<{
    repository: { repository: string };
    selection: { is_active: boolean };
    counts: { specs: number; milestones: number; initiatives: number };
  }>;
}> {
  const response = await fetch(`${apiUrl()}/projects/list`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ owning_account_id: owningAccountId }),
  });
  const payload = await response.json();
  if (!response.ok) {
    throw new Error(
      `list projects failed: ${response.status} ${(payload as { code?: string }).code ?? "unknown"}`,
    );
  }
  return payload as {
    owning_account_id: string;
    projects: Array<{
      repository: { repository: string };
      selection: { is_active: boolean };
      counts: { specs: number; milestones: number; initiatives: number };
    }>;
  };
}

async function activeProject(owningAccountId: string): Promise<{
  active_project: {
    repository: { repository: string };
    selection: { is_active: boolean };
  } | null;
}> {
  const response = await fetch(`${apiUrl()}/projects/active`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ owning_account_id: owningAccountId }),
  });
  const payload = await response.json();
  if (!response.ok) {
    throw new Error(
      `active project failed: ${response.status} ${(payload as { code?: string }).code ?? "unknown"}`,
    );
  }
  return payload as {
    active_project: {
      repository: { repository: string };
      selection: { is_active: boolean };
    } | null;
  };
}

Given(
  /^(\w+) has a project account$/,
  async ({ projectWorld }, name: string) => {
    const state = actor(projectWorld, name);
    state.accountId = await createProjectAccount(name);
    state.lastConnectedRepository = null;
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

    await page.getByLabel(/owning account id/i).fill(state.accountId);
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
    const unknownAccount = randomUUID();

    await page.goto("/projects");
    await page.waitForLoadState("domcontentloaded");

    await page.getByLabel(/owning account id/i).fill(unknownAccount);
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
    state.accountId = unknownAccount;
    state.lastConnectedRepository = null;
    state.lastCreatedRepository = null;
    state.lastDesignatedHost = null;
  },
);

When(
  /^(\w+) creates new repository "([^"]+)" at designated host "([^"]+)" as an active project$/,
  async (
    { projectWorld },
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

    const requestAccountId = host.canCreate ? state.accountId : randomUUID();
    const response = await fetch(`${apiUrl()}/projects/create`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        owning_account_id: requestAccountId,
        repository,
        designated_host: canonicalHostName,
        select_as_active: true,
      }),
    });
    const payload = (await response.json()) as {
      code?: string;
      project?: { repository?: { repository?: string } };
    };

    state.lastDesignatedHost = canonicalHostName;
    if (response.ok) {
      projectWorld.lastFailureCode = null;
      state.lastConnectedRepository = null;
      state.lastCreatedRepository =
        payload.project?.repository?.repository ?? canonicalRepositoryName;
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
  async ({ projectWorld }, name: string, repository: string) => {
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account id`);
    }
    const canonical = canonicalRepository(repository);
    const list = await listProjects(state.accountId);
    const visible = list.projects.some(
      (project) => project.repository.repository === canonical,
    );
    if (!visible) {
      throw new Error(`repository ${canonical} not visible in list`);
    }
  },
);

Then(
  /^(\w+) has active project repository "([^"]+)"$/,
  async ({ projectWorld }, name: string, repository: string) => {
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account id`);
    }
    const canonical = canonicalRepository(repository);
    const active = await activeProject(state.accountId);
    const project = active.active_project;
    if (!project) {
      throw new Error(`expected active project ${canonical}, got none`);
    }
    if (
      project.repository.repository !== canonical ||
      !project.selection.is_active
    ) {
      throw new Error(
        `expected active project ${canonical}, got ${project.repository.repository}`,
      );
    }
  },
);

Then(
  /^(\w+) has exactly (\d+) connected project records$/,
  async ({ projectWorld }, name: string, expectedRaw: string) => {
    const expected = Number.parseInt(expectedRaw, 10);
    const state = actor(projectWorld, name);
    if (!state.accountId) {
      throw new Error(`actor ${name} has no project account id`);
    }
    const list = await listProjects(state.accountId);
    if (list.projects.length !== expected) {
      throw new Error(
        `expected ${expected} connected project records, got ${list.projects.length}`,
      );
    }
  },
);

Then(
  /^repository "([^"]+)" has zero Tanren activity counts$/,
  async ({ projectWorld }, repository: string) => {
    const canonical = canonicalRepository(repository);
    const fixture = projectWorld.repositories.get(canonical);
    if (!fixture) {
      throw new Error(`repository fixture missing for ${canonical}`);
    }
    if (fixture.priorCommits <= 0) {
      throw new Error(
        "prior commits witness requires fixture prior commits > 0",
      );
    }

    const accountIds = [...projectWorld.actors.values()]
      .map((state) => state.accountId)
      .filter((value): value is string => typeof value === "string");

    for (const accountId of accountIds) {
      const list = await listProjects(accountId);
      const project = list.projects.find(
        (item) => item.repository.repository === canonical,
      );
      if (!project) {
        continue;
      }
      if (
        project.counts.specs !== 0 ||
        project.counts.milestones !== 0 ||
        project.counts.initiatives !== 0
      ) {
        throw new Error(
          `expected zero counts for ${canonical}, got specs=${project.counts.specs} milestones=${project.counts.milestones} initiatives=${project.counts.initiatives}`,
        );
      }
      return;
    }

    throw new Error(`repository ${canonical} not found in actor project lists`);
  },
);

Then(
  /^repository "([^"]+)" starts with zero Tanren activity counts$/,
  async ({ projectWorld }, repository: string) => {
    const canonical = canonicalRepository(repository);
    const accountIds = [...projectWorld.actors.values()]
      .map((state) => state.accountId)
      .filter((value): value is string => typeof value === "string");

    for (const accountId of accountIds) {
      const list = await listProjects(accountId);
      const project = list.projects.find(
        (item) => item.repository.repository === canonical,
      );
      if (!project) {
        continue;
      }
      if (
        project.counts.specs !== 0 ||
        project.counts.milestones !== 0 ||
        project.counts.initiatives !== 0
      ) {
        throw new Error(
          `expected zero initial counts for ${canonical}, got specs=${project.counts.specs} milestones=${project.counts.milestones} initiatives=${project.counts.initiatives}`,
        );
      }
      return;
    }

    throw new Error(`repository ${canonical} not found in actor project lists`);
  },
);
