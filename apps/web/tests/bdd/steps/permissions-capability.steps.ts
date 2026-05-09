import { createBdd } from "playwright-bdd";

import { test, type ActorState, type WebWorld } from "./account.steps";
import { resolveAccountIdByEmail } from "../support/test-hooks";

const { When, Then } = createBdd(test);

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";

interface CapabilityMemo {
  canViewMyPermissions: boolean | null;
}

const capabilityMemos = new WeakMap<WebWorld, CapabilityMemo>();

function actor(world: WebWorld, name: string): ActorState {
  let existing = world.actors.get(name);
  if (!existing) {
    existing = {};
    world.actors.set(name, existing);
  }
  return existing;
}

function capabilityMemo(world: WebWorld): CapabilityMemo {
  const existing = capabilityMemos.get(world);
  if (existing) {
    return existing;
  }
  const next: CapabilityMemo = { canViewMyPermissions: null };
  capabilityMemos.set(world, next);
  return next;
}

async function signInActor(
  page: import("@playwright/test").Page,
  world: WebWorld,
  name: string,
): Promise<void> {
  const a = actor(world, name);
  if (!a.email || !a.password) {
    throw new Error(`actor ${name} has no recorded credentials`);
  }
  await page.context().clearCookies();
  await page.goto("/sign-in");
  await page.getByLabel(/email/i).fill(a.email);
  await page.getByLabel(/password/i).fill(a.password);
  await page.getByRole("button", { name: /^sign in$/i }).click();
  await page.waitForURL("/");
  a.hasSession = true;
}

async function cookieHeader(
  page: import("@playwright/test").Page,
): Promise<string> {
  const cookies = await page.context().cookies();
  return cookies.map((c) => `${c.name}=${c.value}`).join("; ");
}

When(
  /^([a-zA-Z]\w*) discovers their self-permissions capability$/,
  async ({ page, world }, name: string) => {
    capabilityMemo(world).canViewMyPermissions = null;
    await signInActor(page, world, name);
    const response = await page.request.get(`${API_URL}/me/capabilities`, {
      headers: {
        cookie: await cookieHeader(page),
      },
    });
    const current = actor(world, name);
    if (!response.ok()) {
      const body = (await response.json()) as { code?: string };
      current.hasSession = false;
      current.lastFailureCode = body.code ?? "unknown";
      capabilityMemo(world).canViewMyPermissions = null;
      return;
    }
    const body = (await response.json()) as {
      can_view_my_permissions: boolean;
    };
    current.hasSession = true;
    delete current.lastFailureCode;
    capabilityMemo(world).canViewMyPermissions = body.can_view_my_permissions;
  },
);

When(
  "an unauthenticated client discovers self-permissions capability",
  async ({ page, world }) => {
    capabilityMemo(world).canViewMyPermissions = null;
    await page.context().clearCookies();
    const response = await page.request.get(`${API_URL}/me/capabilities`);
    const unauth = actor(world, "unauthenticated");
    if (response.ok()) {
      unauth.hasSession = false;
      unauth.lastFailureCode = "unexpected_success";
      capabilityMemo(world).canViewMyPermissions = null;
      return;
    }
    const body = (await response.json()) as { code?: string };
    unauth.hasSession = false;
    unauth.lastFailureCode = body.code ?? "unknown";
    capabilityMemo(world).canViewMyPermissions = null;
  },
);

When(
  /^(\w+) attempts to discover (\w+)'s permissions capability through the self capability view$/,
  async ({ page, world }, actorName: string, targetName: string) => {
    capabilityMemo(world).canViewMyPermissions = null;
    await signInActor(page, world, actorName);
    const target = actor(world, targetName);
    if (!target.email) {
      throw new Error(`target actor ${targetName} has no recorded email`);
    }
    const resolvedAccountId = await resolveAccountIdByEmail(target.email);
    const response = await page.request.get(
      `${API_URL}/me/capabilities?account_id=${encodeURIComponent(resolvedAccountId)}`,
      {
        headers: {
          cookie: await cookieHeader(page),
        },
      },
    );

    const acting = actor(world, actorName);
    if (response.ok()) {
      acting.hasSession = false;
      acting.lastFailureCode = "unexpected_success";
      capabilityMemo(world).canViewMyPermissions = null;
      return;
    }
    const body = (await response.json()) as { code?: string };
    acting.hasSession = false;
    acting.lastFailureCode = body.code ?? "unknown";
    capabilityMemo(world).canViewMyPermissions = null;
  },
);

Then(
  "the capability indicates my permissions navigation is available",
  async ({ world }) => {
    if (capabilityMemo(world).canViewMyPermissions !== true) {
      throw new Error("expected can_view_my_permissions=true");
    }
  },
);

Then(
  /^(\w+) sees the My permissions navigation link on the home page$/,
  async ({ page, world }, name: string) => {
    await signInActor(page, world, name);
    await page.goto("/");
    await page.getByRole("link", { name: /my permissions/i }).waitFor();
  },
);
