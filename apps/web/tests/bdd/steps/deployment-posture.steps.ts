// playwright-bdd step definitions for the `@web` slice of B-0137.
//
// The Web BDD runner uses the same canonical feature file as the Rust
// harness. For posture flows we drive the API from Playwright so we can
// prove cookie-backed auth, shared wire taxonomy, and capability payloads.

import { createBdd } from "playwright-bdd";
import type { Page } from "@playwright/test";

import { test } from "./account.steps";
import {
  assertCapabilitySummary,
  assertCanonicalSupportedPostures,
  decodeCurrentResponse,
  decodeSetResponse,
  decodeSupportedResponse,
  isDeploymentPosture,
  type PostureScenarioState,
  unsupportedPostureFailure,
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

async function signUpActor(
  page: Page,
  world: WebWorld,
  actorName: string,
  email: string,
  password: string,
  displayName: string,
): Promise<string> {
  const response = await page.request.post(`${apiUrl()}/accounts`, {
    data: {
      email,
      password,
      display_name: displayName,
    },
  });
  if (!response.ok()) {
    const parsed = normalizeFailureBody(
      await response.json().catch(() => ({})),
      response.status(),
    );
    throw new Error(
      `sign-up failed for ${actorName}: ${parsed.code} (${parsed.summary})`,
    );
  }
  const data = (await response.json()) as {
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

async function setPosture(
  page: Page,
  world: WebWorld,
  postureRaw: string,
  accountId: string,
): Promise<void> {
  const postureActor = actor(world, "actor");
  const state = stateFor(world);
  if (!isDeploymentPosture(postureRaw)) {
    const failure = unsupportedPostureFailure(postureRaw);
    state.lastFailureSummary = failure.summary;
    postureActor.hasSession = false;
    postureActor.lastFailureCode = failure.code;
    return;
  }

  const response = await browserJsonRequest(
    page,
    "POST",
    "/deployment-postures",
    {
      scope: { scope: "account", account_id: accountId },
      posture: postureRaw,
    },
  );
  if (response.ok) {
    state.lastSet = decodeSetResponse(response.json);
    delete state.lastFailureSummary;
    postureActor.hasSession = true;
    delete postureActor.lastFailureCode;
    return;
  }
  const failure = normalizeFailureBody(response.json, response.status);
  state.lastFailureSummary = failure.summary;
  postureActor.hasSession = false;
  postureActor.lastFailureCode = failure.code;
}

async function readPosture(
  page: Page,
  world: WebWorld,
  accountId: string,
): Promise<void> {
  const postureActor = actor(world, "actor");
  const state = stateFor(world);
  const response = await browserJsonRequest(
    page,
    "GET",
    `/deployment-postures/account/${encodeURIComponent(accountId)}`,
  );
  if (response.ok) {
    decodeCurrentResponse(response.json);
    delete state.lastFailureSummary;
    postureActor.hasSession = true;
    delete postureActor.lastFailureCode;
    return;
  }
  const failure = normalizeFailureBody(response.json, response.status);
  state.lastFailureSummary = failure.summary;
  postureActor.hasSession = false;
  postureActor.lastFailureCode = failure.code;
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
    const response = await browserJsonRequest(
      page,
      "GET",
      "/deployment-postures",
    );
    if (!response.ok) {
      throw new Error(
        `list supported postures failed with HTTP ${response.status}`,
      );
    }
    const data = decodeSupportedResponse(response.json);
    if (!Array.isArray(data.supported)) {
      throw new Error("supported posture payload did not include an array");
    }
    stateFor(world).lastSupported = data.supported;
  },
);

When(
  "the actor sets deployment posture {string} for their account scope over web",
  async ({ page, world }, posture: string) => {
    const state = stateFor(world);
    if (!state.actorAccountId) {
      throw new Error("actor account id missing before posture set");
    }
    await setPosture(page, world, posture, state.actorAccountId);
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
    await setPosture(page, world, posture, state.otherAccountId);
  },
);

When(
  "the actor sets deployment posture {string} for a missing account scope over web",
  async ({ page, world }, posture: string) => {
    await setPosture(page, world, posture, crypto.randomUUID());
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
    await readPosture(page, world, state.otherAccountId);
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
  async ({ world }, posture: string) => {
    const last = stateFor(world).lastSet;
    if (!last) {
      throw new Error("no posture-set response recorded");
    }
    if (last.posture !== posture) {
      throw new Error(`expected posture ${posture}, got ${last.posture}`);
    }
  },
);

Then(
  "the web view shows available and unavailable capability summaries",
  async ({ world }) => {
    const last = stateFor(world).lastSet;
    if (!last) {
      throw new Error("no posture capability summary recorded");
    }
    if (
      last.capability_summary.available.length === 0 ||
      last.capability_summary.unavailable.length === 0
    ) {
      throw new Error(
        "expected non-empty available and unavailable capability lists",
      );
    }
  },
);

Then(
  "the recorded posture for the actor account over web is {string}",
  async ({ page, world }, posture: string) => {
    const state = stateFor(world);
    if (!state.actorAccountId) {
      throw new Error("actor account id missing before readback");
    }
    const response = await browserJsonRequest(
      page,
      "GET",
      `/deployment-postures/account/${encodeURIComponent(state.actorAccountId)}`,
    );
    if (!response.ok) {
      throw new Error(
        `get deployment posture failed with HTTP ${response.status}`,
      );
    }
    const payload = decodeCurrentResponse(response.json);
    if (!payload.current) {
      throw new Error("expected an existing recorded posture");
    }
    if (payload.current.posture !== posture) {
      throw new Error(
        `expected recorded posture ${posture}, got ${payload.current.posture}`,
      );
    }
  },
);

Then(
  "recent events attribute the posture change to the actor with posture {string}",
  async ({ page, world }, posture: string) => {
    const actorId = stateFor(world).actorAccountId;
    if (!actorId) {
      throw new Error(
        "actor account id missing before event attribution check",
      );
    }

    for (let attempt = 0; attempt < 5; attempt += 1) {
      const events = await readRecentEvents(page, 40);
      const matched = events.some((event) => {
        const payload = event.payload?.payload;
        return (
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
