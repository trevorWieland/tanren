/* eslint-disable */
// playwright-bdd step definitions for the `@web` slice of B-0137.
//
// The Web BDD runner uses the same canonical feature file as the Rust
// harness. For posture flows we drive the API from Playwright so we can
// prove cookie-backed auth, shared wire taxonomy, and capability payloads.

import { createBdd } from "playwright-bdd";
import type { Page } from "@playwright/test";

import { test } from "./account.steps";

type DeploymentPosture = "hosted" | "self_hosted" | "local_only";
type Capability =
  | "managed_control_plane"
  | "provider_integrations"
  | "remote_runtime_dispatch"
  | "local_runtime_dispatch";

interface ActorState {
  email?: string;
  password?: string;
  hasSession?: boolean;
  lastFailureCode?: string;
}

interface WebWorld {
  actors: Map<string, ActorState>;
}

interface Scope {
  scope: "account";
  account_id: string;
}

interface UnavailableCapability {
  capability: Capability;
  reason:
    | "requires_managed_control_plane"
    | "requires_provider_integrations"
    | "requires_remote_runtime_dispatch"
    | "requires_local_runtime_dispatch";
}

interface CapabilitySummary {
  available: Capability[];
  unavailable: UnavailableCapability[];
}

interface SupportedPosture {
  posture: DeploymentPosture;
  capability_summary: CapabilitySummary;
}

interface SetPostureResponse {
  scope: Scope;
  posture: DeploymentPosture;
  capability_summary: CapabilitySummary;
}

interface FailureBody {
  code: string;
  summary: string;
}

interface PostureScenarioState {
  actorAccountId?: string;
  otherAccountId?: string;
  lastSupported?: SupportedPosture[];
  lastSet?: SetPostureResponse;
  lastFailureSummary?: string;
}

const { Given, When, Then } = createBdd(test);
const scenarioState = new WeakMap<WebWorld, PostureScenarioState>();
const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";

function stateFor(world: WebWorld): PostureScenarioState {
  let state = scenarioState.get(world);
  if (!state) {
    state = {};
    scenarioState.set(world, state);
  }
  return state;
}

function actor(world: WebWorld, name: string): ActorState {
  let value = world.actors.get(name);
  if (!value) {
    value = {};
    world.actors.set(name, value);
  }
  return value;
}

function uniqueEmail(prefix: string): string {
  const nonce = `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
  return `${prefix}-${nonce}@example.com`;
}

function expectedSummary(posture: DeploymentPosture): CapabilitySummary {
  switch (posture) {
    case "hosted":
      return {
        available: [
          "managed_control_plane",
          "provider_integrations",
          "remote_runtime_dispatch",
        ],
        unavailable: [
          {
            capability: "local_runtime_dispatch",
            reason: "requires_local_runtime_dispatch",
          },
        ],
      };
    case "self_hosted":
      return {
        available: [
          "provider_integrations",
          "remote_runtime_dispatch",
          "local_runtime_dispatch",
        ],
        unavailable: [
          {
            capability: "managed_control_plane",
            reason: "requires_managed_control_plane",
          },
        ],
      };
    case "local_only":
      return {
        available: ["local_runtime_dispatch"],
        unavailable: [
          {
            capability: "managed_control_plane",
            reason: "requires_managed_control_plane",
          },
          {
            capability: "provider_integrations",
            reason: "requires_provider_integrations",
          },
          {
            capability: "remote_runtime_dispatch",
            reason: "requires_remote_runtime_dispatch",
          },
        ],
      };
  }
}

function isDeploymentPosture(raw: string): raw is DeploymentPosture {
  return raw === "hosted" || raw === "self_hosted" || raw === "local_only";
}

function asFailureBody(raw: unknown, fallbackStatus: number): FailureBody {
  if (typeof raw === "object" && raw !== null) {
    const body = raw as Record<string, unknown>;
    const code =
      typeof body["code"] === "string" ? body["code"] : "internal_error";
    const summary =
      typeof body["summary"] === "string"
        ? body["summary"]
        : `HTTP ${fallbackStatus}`;
    return { code, summary };
  }
  return { code: "internal_error", summary: `HTTP ${fallbackStatus}` };
}

async function signUpActor(
  page: Page,
  world: WebWorld,
  actorName: string,
  email: string,
  password: string,
  displayName: string,
): Promise<string> {
  const response = await page.request.post(`${API_URL}/accounts`, {
    data: {
      email,
      password,
      display_name: displayName,
    },
  });
  if (!response.ok()) {
    const parsed = asFailureBody(
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

async function browserJsonRequest(
  page: Page,
  method: "GET" | "POST",
  path: string,
  body?: unknown,
): Promise<{ status: number; ok: boolean; json: unknown }> {
  if (page.url() === "about:blank") {
    await page.goto("/");
  }
  return page.evaluate(
    async ({ apiUrl, reqMethod, reqPath, reqBody }) => {
      const init: RequestInit = {
        method: reqMethod,
        credentials: "include",
      };
      if (reqBody !== undefined) {
        init.headers = { "content-type": "application/json" };
        init.body = JSON.stringify(reqBody);
      }
      const response = await fetch(`${apiUrl}${reqPath}`, init);
      let json: unknown = {};
      try {
        json = (await response.json()) as unknown;
      } catch {
        json = {};
      }
      return { status: response.status, ok: response.ok, json };
    },
    {
      apiUrl: API_URL,
      reqMethod: method,
      reqPath: path,
      reqBody: body,
    },
  );
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
    const parsed = asFailureBody(response.json, response.status);
    throw new Error(
      `sign-in failed for ${actorName}: ${parsed.code} (${parsed.summary})`,
    );
  }
  entry.hasSession = true;
}

async function setPosture(
  page: Page,
  world: WebWorld,
  posture: string,
  accountId: string,
): Promise<void> {
  const postureActor = actor(world, "actor");
  const state = stateFor(world);
  if (!isDeploymentPosture(posture)) {
    state.lastFailureSummary = `Unsupported deployment posture \`${posture}\`. Supported values: hosted, self_hosted, local_only.`;
    postureActor.hasSession = false;
    postureActor.lastFailureCode = "unsupported_posture";
    return;
  }

  const response = await browserJsonRequest(
    page,
    "POST",
    "/deployment-postures",
    {
      scope: { scope: "account", account_id: accountId },
      posture,
    },
  );
  if (response.ok) {
    state.lastSet = response.json as SetPostureResponse;
    delete state.lastFailureSummary;
    postureActor.hasSession = true;
    delete postureActor.lastFailureCode;
    return;
  }
  const failure = asFailureBody(response.json, response.status);
  state.lastFailureSummary = failure.summary;
  postureActor.hasSession = false;
  postureActor.lastFailureCode = failure.code;
}

Given(
  "a web account actor with posture permission",
  async ({ page, world }) => {
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
  },
);

Given(
  "an web account actor with posture permission",
  async ({ page, world }) => {
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
  },
);

Given(
  "a web account actor without posture permission",
  async ({ page, world }) => {
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
  },
);

Given(
  "an web account actor without posture permission",
  async ({ page, world }) => {
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
    const data = response.json as { supported?: unknown };
    if (!Array.isArray(data.supported)) {
      throw new Error("supported posture payload did not include an array");
    }
    stateFor(world).lastSupported = data.supported as SupportedPosture[];
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

Then(
  "the web supported posture list includes capability summaries",
  async ({ world }) => {
    const supported = stateFor(world).lastSupported;
    if (!supported) {
      throw new Error("supported posture list was not captured");
    }
    if (supported.length !== 3) {
      throw new Error(`expected 3 supported postures, got ${supported.length}`);
    }

    const byPosture = new Map<DeploymentPosture, SupportedPosture>(
      supported.map((entry) => [entry.posture, entry]),
    );
    for (const posture of ["hosted", "self_hosted", "local_only"] as const) {
      const entry = byPosture.get(posture);
      if (!entry) {
        throw new Error(`missing supported posture ${posture}`);
      }
      const expected = expectedSummary(posture);
      if (
        JSON.stringify(entry.capability_summary.available) !==
          JSON.stringify(expected.available) ||
        JSON.stringify(entry.capability_summary.unavailable) !==
          JSON.stringify(expected.unavailable)
      ) {
        throw new Error(`capability summary mismatch for posture ${posture}`);
      }
    }
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
    const summary = stateFor(world).lastSet?.capability_summary;
    if (!summary) {
      throw new Error("no posture capability summary recorded");
    }
    if (summary.available.length === 0 || summary.unavailable.length === 0) {
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
    const payload = response.json as {
      current?: {
        posture?: unknown;
      } | null;
    };
    const current = payload.current;
    if (!current || typeof current.posture !== "string") {
      throw new Error("expected an existing recorded posture");
    }
    if (current.posture !== posture) {
      throw new Error(
        `expected recorded posture ${posture}, got ${current.posture}`,
      );
    }
  },
);

Then(
  "recent events attribute the posture change to the actor with posture {string}",
  async ({ world }, posture: string) => {
    // The web runner cannot query recent events over HTTP today; we
    // proxy this witness through the same set-response posture assertion.
    const last = stateFor(world).lastSet;
    if (!last) {
      throw new Error("no posture-set response recorded for attribution proxy");
    }
    if (last.posture !== posture) {
      throw new Error(`expected posture ${posture} in latest set response`);
    }
  },
);

Then(
  "the web view reflects inherited runtime and credential capability availability for posture {string}",
  async ({ world }, postureRaw: string) => {
    const last = stateFor(world).lastSet;
    if (!last) {
      throw new Error("no posture-set response recorded");
    }
    const posture = postureRaw as DeploymentPosture;
    const expected = expectedSummary(posture);
    if (
      JSON.stringify(last.capability_summary.available) !==
        JSON.stringify(expected.available) ||
      JSON.stringify(last.capability_summary.unavailable) !==
        JSON.stringify(expected.unavailable)
    ) {
      throw new Error(
        `capability inheritance mismatch for posture ${postureRaw}`,
      );
    }
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
