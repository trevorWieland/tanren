// playwright-bdd step definitions for the `@web` slice of B-0137.
//
// The Web BDD runner uses the same canonical feature file as the Rust
// harness. For posture flows we now drive the real `/deployment-posture`
// page workflow and assert against the resulting wire responses.

import { createBdd } from "playwright-bdd";
import {
  expect,
  type Locator,
  type Page,
  type Request,
  type Response,
  type Route,
} from "@playwright/test";

import { test } from "./account.steps";
import {
  assertCapabilitySummary,
  assertCanonicalSupportedPostures,
  decodeCurrentResponse,
  decodeSetResponse,
  decodeSupportedResponse,
  isDeploymentPosture,
  type PostureScenarioState,
} from "./support/deployment-posture";
import {
  actor,
  apiUrl,
  browserJsonRequest,
  normalizeFailureBody,
  uniqueEmail,
  type WebWorld,
} from "./support/web-wire";

const { Given, When, Then } = createBdd(test);
const scenarioState = new WeakMap<WebWorld, PostureScenarioState>();

function stateFor(world: WebWorld): PostureScenarioState {
  let state = scenarioState.get(world);
  if (!state) {
    state = {};
    scenarioState.set(world, state);
  }
  return state;
}

function escapeRegexLiteral(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function readbackSection(page: Page): Locator {
  return page
    .locator("section")
    .filter({
      has: page.getByRole("heading", { name: /capability readback/i }),
    })
    .first();
}

function currentPostureCard(page: Page): Locator {
  return readbackSection(page)
    .locator("div")
    .filter({
      hasText: /scope:\s*(account|project|installation):/i,
    })
    .first();
}

function auditSection(page: Page): Locator {
  return page
    .locator("section")
    .filter({
      has: page.getByRole("heading", { name: /audit/i }),
    })
    .first();
}

function capabilityRow(
  currentCard: Locator,
  label: "available" | "unavailable",
): Locator {
  return currentCard
    .locator("p")
    .filter({ hasText: new RegExp(`^${label}:`, "i") })
    .first();
}

function parseCapabilityRowValues(
  rowText: string,
  label: "available" | "unavailable",
): string[] {
  const match = rowText.match(new RegExp(`^${label}:\\s*(.+)$`, "i"));
  if (!match) {
    throw new Error(`${label} capability row was not readable: ${rowText}`);
  }
  const valuesText = match[1];
  if (valuesText === undefined) {
    throw new Error(`${label} capability row had no values: ${rowText}`);
  }
  return valuesText
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => entry !== "");
}

function operationalScopeLabelFromCurrentScope(
  scope:
    | {
        scope: "account";
        account_id: string;
      }
    | {
        scope: "project";
        project_id: string;
      }
    | {
        scope: "installation";
        installation_id: string;
      },
): string {
  if (scope.scope === "account") {
    return `account: ${scope.account_id}`;
  }
  if (scope.scope === "project") {
    return `project: ${scope.project_id}`;
  }
  return `installation: ${scope.installation_id}`;
}

function isListPostureResponse(response: Response): boolean {
  if (response.request().method() !== "GET") {
    return false;
  }
  const pathname = new URL(response.url()).pathname;
  return pathname === "/deployment-postures";
}

function isSetPostureResponse(response: Response): boolean {
  if (response.request().method() !== "POST") {
    return false;
  }
  const pathname = new URL(response.url()).pathname;
  return pathname === "/deployment-postures";
}

function isGetPostureResponse(response: Response): boolean {
  if (response.request().method() !== "GET") {
    return false;
  }
  const pathname = new URL(response.url()).pathname;
  return pathname.startsWith("/deployment-postures/");
}

async function parseJsonResponse(response: Response): Promise<{
  ok: boolean;
  status: number;
  json: unknown;
}> {
  let json: unknown = {};
  try {
    json = (await response.json()) as unknown;
  } catch {
    json = {};
  }
  return {
    ok: response.ok(),
    status: response.status(),
    json,
  };
}

async function openPosturePageForScope(
  page: Page,
  accountId: string,
): Promise<void> {
  await page.addInitScript(
    `window.localStorage.setItem("tanren.active-account-scope-id", ${JSON.stringify(accountId)});`,
  );
  await page.goto("/deployment-posture");
  await page
    .getByRole("heading", { name: /deployment posture operations/i })
    .waitFor();
}

async function discoverCapabilitiesFromUi(page: Page): Promise<unknown> {
  const listResponsePromise = page.waitForResponse(isListPostureResponse);
  await page
    .getByRole("button", { name: /discover capabilities/i })
    .first()
    .click();
  const listResponse = await parseJsonResponse(await listResponsePromise);
  const operationalScopeSelect = page.locator("#posture-operational-scope");
  const postureSelect = page.locator("#posture-value");
  const loadButton = page.getByRole("button", { name: /load/i }).first();
  const saveButton = page.getByRole("button", { name: /save/i }).first();

  await operationalScopeSelect.waitFor();
  await postureSelect.waitFor();
  await expect(operationalScopeSelect).toBeEnabled();
  await expect(postureSelect).toBeEnabled();
  await expect(loadButton).toBeEnabled();
  await expect(saveButton).toBeEnabled();
  await page.waitForFunction(
    () => {
      const element =
        document.querySelector<HTMLSelectElement>("#posture-value");
      return element !== null && element.options.length > 0;
    },
    undefined,
    { timeout: 10_000 },
  );
  if (!listResponse.ok) {
    throw new Error(
      `list supported postures failed with HTTP ${listResponse.status}`,
    );
  }
  return listResponse.json;
}

async function setPostureViaUi(
  page: Page,
  world: WebWorld,
  postureRaw: string,
  scopeAccountId: string,
  scopeOverride?: unknown,
): Promise<void> {
  const postureActor = actor(world, "actor");
  const state = stateFor(world);

  await openPosturePageForScope(page, scopeAccountId);
  const supportedPayload = await discoverCapabilitiesFromUi(page);
  const supported = decodeSupportedResponse(supportedPayload);
  state.lastSupported = supported.supported;

  const postureSelect = page.locator("#posture-value");
  await postureSelect.waitFor();
  const shouldTamperRequest =
    !isDeploymentPosture(postureRaw) || scopeOverride !== undefined;
  if (shouldTamperRequest) {
    const replacement = supported.supported[0]?.posture;
    if (!replacement) {
      throw new Error("supported posture list was empty before save");
    }
    await postureSelect.selectOption(replacement);
  } else {
    await postureSelect.selectOption(postureRaw);
  }

  const routePattern = "**/deployment-postures";
  let routeHandler: ((route: Route, request: Request) => Promise<void>) | null =
    null;
  if (shouldTamperRequest) {
    routeHandler = async (route: Route, request: Request): Promise<void> => {
      if (request.method() !== "POST") {
        await route.continue();
        return;
      }
      let payload: unknown = {};
      try {
        payload = request.postDataJSON();
      } catch {
        payload = {};
      }
      const payloadObject: Record<string, unknown> =
        typeof payload === "object" && payload !== null
          ? (payload as Record<string, unknown>)
          : {};
      const nextPayload: Record<string, unknown> = { ...payloadObject };
      if (!isDeploymentPosture(postureRaw)) {
        nextPayload["posture"] = postureRaw;
      }
      if (scopeOverride !== undefined) {
        nextPayload["scope"] = scopeOverride;
      }
      await route.continue({
        headers: {
          ...request.headers(),
          "content-type": "application/json",
        },
        postData: JSON.stringify(nextPayload),
      });
    };
    await page.route(routePattern, routeHandler);
  }

  const setResponsePromise = page.waitForResponse(isSetPostureResponse);
  try {
    await page
      .getByRole("button", { name: /(set posture|save)/i })
      .first()
      .click();
  } finally {
    if (routeHandler) {
      await page.unroute(routePattern, routeHandler);
    }
  }
  const setResponse = await parseJsonResponse(await setResponsePromise);

  if (setResponse.ok) {
    state.lastSet = decodeSetResponse(setResponse.json);
    delete state.lastFailureSummary;
    postureActor.hasSession = true;
    delete postureActor.lastFailureCode;
    return;
  }

  const failure = normalizeFailureBody(setResponse.json, setResponse.status);
  state.lastFailureSummary = failure.summary;
  postureActor.hasSession = false;
  postureActor.lastFailureCode = failure.code;
}

async function readPostureViaUi(
  page: Page,
  world: WebWorld,
  scopeAccountId: string,
): Promise<void> {
  const postureActor = actor(world, "actor");
  const state = stateFor(world);

  await openPosturePageForScope(page, scopeAccountId);
  const supportedPayload = await discoverCapabilitiesFromUi(page);
  const supported = decodeSupportedResponse(supportedPayload);
  state.lastSupported = supported.supported;

  const getResponsePromise = page.waitForResponse(isGetPostureResponse);
  await page.getByRole("button", { name: /load/i }).first().click();
  const getResponse = await parseJsonResponse(await getResponsePromise);

  if (getResponse.ok) {
    decodeCurrentResponse(getResponse.json);
    delete state.lastFailureSummary;
    postureActor.hasSession = true;
    delete postureActor.lastFailureCode;
    return;
  }

  const failure = normalizeFailureBody(getResponse.json, getResponse.status);
  state.lastFailureSummary = failure.summary;
  postureActor.hasSession = false;
  postureActor.lastFailureCode = failure.code;
}

async function signUpActor(
  page: Page,
  world: WebWorld,
  actorName: string,
  email: string,
  password: string,
  displayName: string,
): Promise<string> {
  const response = await browserJsonRequest(page, "POST", "/accounts", {
    email,
    password,
    display_name: displayName,
  });
  if (!response.ok) {
    const parsed = normalizeFailureBody(response.json, response.status);
    throw new Error(
      `sign-up failed for ${actorName}: ${parsed.code} (${parsed.summary})`,
    );
  }
  const data = response.json as {
    account?: { id?: unknown };
  };
  const accountId = data.account?.id;
  if (typeof accountId !== "string" || accountId === "") {
    throw new Error(
      `sign-up response for ${actorName} did not include account.id`,
    );
  }

  const entry = actor(world, actorName);
  entry.email = email;
  entry.password = password;
  entry.hasSession = true;
  delete entry.lastFailureCode;
  return accountId;
}

async function signInActor(
  page: Page,
  world: WebWorld,
  actorName: string,
): Promise<void> {
  const entry = actor(world, actorName);
  if (!entry.email || !entry.password) {
    throw new Error(`missing credentials for actor ${actorName}`);
  }
  const response = await browserJsonRequest(page, "POST", "/sessions", {
    email: entry.email,
    password: entry.password,
  });
  if (!response.ok) {
    const parsed = normalizeFailureBody(response.json, response.status);
    throw new Error(
      `sign-in failed for ${actorName}: ${parsed.code} (${parsed.summary})`,
    );
  }
  entry.hasSession = true;
}

async function seedActorWithPermission(
  page: Page,
  world: WebWorld,
): Promise<void> {
  const state = stateFor(world);
  const actorEmail = uniqueEmail("actor-web-posture");
  state.actorAccountId = await signUpActor(
    page,
    world,
    "actor",
    actorEmail,
    "actor-posture-pw",
    "Posture actor",
  );
  delete state.lastFailureSummary;
}

async function seedActorWithoutPermission(
  page: Page,
  world: WebWorld,
): Promise<void> {
  const state = stateFor(world);
  await seedActorWithPermission(page, world);

  const otherEmail = uniqueEmail("other-web-posture");
  state.otherAccountId = await signUpActor(
    page,
    world,
    "other",
    otherEmail,
    "other-posture-pw",
    "Other account",
  );

  await page.context().clearCookies();
  await signInActor(page, world, "actor");
  delete state.lastFailureSummary;
}

interface EventEnvelope {
  id?: string;
  payload?: {
    family?: string;
    kind?: string;
    payload?: {
      changed_by?: string;
      posture?: string;
    };
  };
}

async function readRecentEvents(
  page: Page,
  limit: number,
): Promise<EventEnvelope[]> {
  const response = await page.request.get(
    `${apiUrl()}/test-hooks/events/recent?limit=${encodeURIComponent(String(limit))}`,
  );
  if (!response.ok()) {
    throw new Error(`recent event read failed with HTTP ${response.status()}`);
  }
  const payload = (await response.json()) as { events?: unknown };
  if (!Array.isArray(payload.events)) {
    throw new Error("recent event payload did not include an events array");
  }
  return payload.events as EventEnvelope[];
}

Given(
  "a web account actor with posture permission",
  async ({ page, world }) => {
    await seedActorWithPermission(page, world);
  },
);

Given(
  "an web account actor with posture permission",
  async ({ page, world }) => {
    await seedActorWithPermission(page, world);
  },
);

Given(
  "a web account actor without posture permission",
  async ({ page, world }) => {
    await seedActorWithoutPermission(page, world);
  },
);

Given(
  "an web account actor without posture permission",
  async ({ page, world }) => {
    await seedActorWithoutPermission(page, world);
  },
);

When(
  "the actor lists supported deployment postures over web",
  async ({ page, world }) => {
    const state = stateFor(world);
    const accountId = state.actorAccountId;
    if (!accountId) {
      throw new Error("actor account id missing before listing postures");
    }
    await openPosturePageForScope(page, accountId);
    const payload = await discoverCapabilitiesFromUi(page);
    const data = decodeSupportedResponse(payload);
    if (!Array.isArray(data.supported)) {
      throw new Error("supported posture payload did not include an array");
    }
    state.lastSupported = data.supported;
  },
);

When(
  "the actor sets deployment posture {string} for their account scope over web",
  async ({ page, world }, posture: string) => {
    const state = stateFor(world);
    if (!state.actorAccountId) {
      throw new Error("actor account id missing before posture set");
    }
    await setPostureViaUi(page, world, posture, state.actorAccountId);
  },
);

When(
  "the actor sets deployment posture {string} for another account scope over web",
  async ({ page, world }, posture: string) => {
    const state = stateFor(world);
    if (!state.otherAccountId) {
      throw new Error(
        "other account id missing before unauthorized posture set",
      );
    }
    await setPostureViaUi(page, world, posture, state.otherAccountId);
  },
);

When(
  "the actor sets deployment posture {string} for a missing account scope over web",
  async ({ page, world }, posture: string) => {
    await setPostureViaUi(page, world, posture, crypto.randomUUID());
  },
);

When(
  "the actor sets deployment posture {string} for a malformed account scope over web",
  async ({ page, world }, posture: string) => {
    const state = stateFor(world);
    if (!state.actorAccountId) {
      throw new Error(
        "actor account id missing before malformed scope posture set",
      );
    }
    await setPostureViaUi(page, world, posture, state.actorAccountId, {
      scope: "account",
      account_id: "not-a-uuid",
    });
  },
);

When(
  "the actor reads deployment posture for another account scope over web",
  async ({ page, world }) => {
    const state = stateFor(world);
    if (!state.otherAccountId) {
      throw new Error(
        "other account id missing before unauthorized posture read",
      );
    }
    await readPostureViaUi(page, world, state.otherAccountId);
  },
);

Then(
  "the web supported posture list includes capability summaries",
  async ({ world }) => {
    const supported = stateFor(world).lastSupported;
    if (!supported) {
      throw new Error("supported posture list was not captured");
    }
    assertCanonicalSupportedPostures(supported);
  },
);

Then(
  "the web view shows posture {string}",
  async ({ page }, posture: string) => {
    const currentCard = currentPostureCard(page);
    await expect(currentCard).toBeVisible();
    await expect(currentCard.locator("p.font-mono").first()).toHaveText(
      new RegExp(`^${escapeRegexLiteral(posture)}$`),
    );
  },
);

Then(
  "the web view shows available and unavailable capability summaries",
  async ({ page }) => {
    const currentCard = currentPostureCard(page);
    await expect(currentCard).toBeVisible();
    const availableRow = capabilityRow(currentCard, "available");
    const unavailableRow = capabilityRow(currentCard, "unavailable");
    await expect(availableRow).toBeVisible();
    await expect(unavailableRow).toBeVisible();

    const availableText = ((await availableRow.textContent()) ?? "").trim();
    const unavailableText = ((await unavailableRow.textContent()) ?? "").trim();
    const availableValues = parseCapabilityRowValues(
      availableText,
      "available",
    );
    const unavailableValues = parseCapabilityRowValues(
      unavailableText,
      "unavailable",
    );

    if (availableValues.length === 0) {
      throw new Error("expected at least one available capability in readback");
    }
    if (unavailableValues.length === 0) {
      throw new Error(
        "expected at least one unavailable capability in readback",
      );
    }

    for (const entry of unavailableValues) {
      if (!/^[a-z_]+ \([a-z_]+\)$/i.test(entry)) {
        throw new Error(
          `unavailable capability entry must include reason text, got: ${entry}`,
        );
      }
    }
  },
);

Then("the web view includes an audit reference", async ({ page }) => {
  const audit = auditSection(page);
  await expect(audit).toBeVisible();
  const auditReferenceRow = audit
    .locator("p")
    .filter({ hasText: /^audit reference:/i })
    .first();
  await expect(auditReferenceRow).toBeVisible();
  const auditReferenceText = ((await auditReferenceRow.textContent()) ?? "")
    .replace(/^audit reference:\s*/i, "")
    .trim();
  if (auditReferenceText === "") {
    throw new Error("expected a non-empty rendered audit reference");
  }
});

Then(
  "the recorded posture for the actor account over web is {string}",
  async ({ page, world }, posture: string) => {
    const state = stateFor(world);
    if (!state.actorAccountId) {
      throw new Error("actor account id missing before readback");
    }
    await openPosturePageForScope(page, state.actorAccountId);
    await discoverCapabilitiesFromUi(page);
    const getResponsePromise = page.waitForResponse(isGetPostureResponse);
    await page.getByRole("button", { name: /load/i }).first().click();
    const getResponse = await parseJsonResponse(await getResponsePromise);
    if (!getResponse.ok) {
      throw new Error(
        `get deployment posture failed with HTTP ${getResponse.status}`,
      );
    }
    const payload = decodeCurrentResponse(getResponse.json);
    if (!payload.current) {
      throw new Error("expected an existing recorded posture");
    }
    if (payload.current.posture !== posture) {
      throw new Error(
        `expected recorded posture ${posture}, got ${payload.current.posture}`,
      );
    }

    const currentCard = currentPostureCard(page);
    await expect(currentCard).toBeVisible();
    await expect(currentCard.locator("p.font-mono").first()).toHaveText(
      new RegExp(`^${escapeRegexLiteral(posture)}$`),
    );

    const expectedScopeLabel = operationalScopeLabelFromCurrentScope(
      payload.current.scope,
    );
    await expect(currentCard).toContainText(
      new RegExp(`scope:\\s*${escapeRegexLiteral(expectedScopeLabel)}`, "i"),
    );

    const availableRow = capabilityRow(currentCard, "available");
    const unavailableRow = capabilityRow(currentCard, "unavailable");
    await expect(availableRow).toBeVisible();
    await expect(unavailableRow).toBeVisible();

    const availableText = ((await availableRow.textContent()) ?? "").trim();
    const unavailableText = ((await unavailableRow.textContent()) ?? "").trim();
    const availableValues = parseCapabilityRowValues(
      availableText,
      "available",
    );
    const unavailableValues = parseCapabilityRowValues(
      unavailableText,
      "unavailable",
    );

    if (
      availableValues.length !==
      payload.current.capability_summary.available.length
    ) {
      throw new Error(
        "available capability count in readback did not match wire response",
      );
    }
    if (
      unavailableValues.length !==
      payload.current.capability_summary.unavailable.length
    ) {
      throw new Error(
        "unavailable capability count in readback did not match wire response",
      );
    }

    for (const capability of payload.current.capability_summary.available) {
      await expect(availableRow).toContainText(
        new RegExp(`\\b${escapeRegexLiteral(capability)}\\b`),
      );
    }
    for (const capability of payload.current.capability_summary.unavailable) {
      await expect(unavailableRow).toContainText(
        `${capability.capability} (${capability.reason})`,
      );
    }
    for (const entry of unavailableValues) {
      if (!/^[a-z_]+ \([a-z_]+\)$/i.test(entry)) {
        throw new Error(
          `unavailable capability entry must include reason text, got: ${entry}`,
        );
      }
    }
  },
);

Then(
  "recent events attribute the posture change to the actor with posture {string}",
  async ({ page, world }, posture: string) => {
    const state = stateFor(world);
    const actorId = state.actorAccountId;
    const auditReference = state.lastSet?.audit_reference;
    if (!actorId) {
      throw new Error(
        "actor account id missing before event attribution check",
      );
    }
    if (!auditReference || auditReference.trim() === "") {
      throw new Error("audit reference missing before event attribution check");
    }

    for (let attempt = 0; attempt < 5; attempt += 1) {
      const events = await readRecentEvents(page, 40);
      const matched = events.some((event) => {
        const payload = event.payload?.payload;
        return (
          event.id === auditReference &&
          event.payload?.family === "deployment_posture" &&
          event.payload?.kind === "changed" &&
          payload?.changed_by === actorId &&
          payload?.posture === posture
        );
      });
      if (matched) {
        return;
      }
      await page.waitForTimeout(50);
    }

    throw new Error("expected attributed deployment posture event");
  },
);

Then(
  "the web view reflects inherited runtime and credential capability availability for posture {string}",
  async ({ world }, postureRaw: string) => {
    const last = stateFor(world).lastSet;
    if (!last) {
      throw new Error("no posture-set response recorded");
    }
    if (!isDeploymentPosture(postureRaw)) {
      throw new Error(`unsupported posture under test: ${postureRaw}`);
    }
    assertCapabilitySummary(
      postureRaw,
      last.capability_summary,
      "set response",
    );
  },
);

Then("the error summary is readable", async ({ world }) => {
  const summary = stateFor(world).lastFailureSummary ?? "";
  if (summary.trim() === "") {
    throw new Error("expected a non-empty readable error summary");
  }
  if (!/[a-z]/i.test(summary)) {
    throw new Error("error summary should contain alphabetic characters");
  }
});
