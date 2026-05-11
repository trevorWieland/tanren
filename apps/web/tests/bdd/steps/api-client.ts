import type { Page, Response } from "@playwright/test";

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
  const signInResponsePromise = waitForRouteResponse(page, "POST", "/sessions");

  await page.getByRole("button", { name: /^sign in$/i }).click();

  const result = await Promise.race([
    signInResponsePromise.then(() => "response" as const),
    page
      .locator('form [role="alert"]')
      .first()
      .waitFor({ state: "visible" })
      .then(() => "alert" as const),
  ]);

  if (result === "alert") {
    a.hasSession = false;
    a.lastFailureCode = await classifyFailureFromAlert(page);
    throw new Error(`sign-in failed for ${name} via web UI`);
  }

  const response = await signInResponsePromise;
  const responseBody = parseJson(await response.text());
  const failureCode = failureCodeFromBody(responseBody);
  const hasSessionCookie =
    (await hasSessionCookieInContext(page)) || hasSessionCookieHeader(response);
  const hasCookieSessionEnvelope = isCookieSessionEnvelope(responseBody);

  if (
    !response.ok() ||
    (!hasSessionCookie && !hasCookieSessionEnvelope) ||
    failureCode !== undefined
  ) {
    a.hasSession = false;
    a.lastFailureCode =
      failureCode ?? (await classifyFailureCode(page, responseBody));
    throw new Error(`sign-in failed for ${name} via web UI`);
  }

  a.hasSession = true;
  delete a.lastFailureCode;
}

export async function seedInvitationForOrg(
  token: string,
  orgId: string,
): Promise<void> {
  const apiUrl = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
  const expiresAt = new Date(Date.now() + 365 * 24 * 60 * 60 * 1000);
  const res = await fetch(`${apiUrl}/test-hooks/invitations`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      token,
      expires_at: expiresAt.toISOString(),
      inviting_org_id: orgId,
    }),
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(
      `seed invitation '${token}' for org ${orgId} failed: ${res.status} ${body}`,
    );
  }
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

async function classifyFailureCode(
  page: Page,
  responseBody: unknown,
): Promise<string> {
  return failureCodeFromBody(responseBody) ?? classifyFailureFromAlert(page);
}

function parseJson(text: string): unknown {
  if (text.trim() === "") {
    return null;
  }

  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

function hasSessionCookieHeader(response: Response): boolean {
  const value = response.headers()["set-cookie"];
  if (typeof value !== "string") {
    return false;
  }

  return value
    .split(",")
    .some((header) => header.trimStart().startsWith("tanren_session="));
}

async function hasSessionCookieInContext(page: Page): Promise<boolean> {
  const cookies = await page.context().cookies();
  return cookies.some((cookie) => cookie.name === "tanren_session");
}

function isCookieSessionEnvelope(payload: unknown): boolean {
  if (!isRecord(payload)) {
    return false;
  }

  const session = payload["session"];
  if (!isRecord(session)) {
    return false;
  }

  if (session["transport"] !== "cookie") {
    return false;
  }

  return (
    typeof session["account_id"] === "string" &&
    typeof session["expires_at"] === "string"
  );
}

function failureCodeFromBody(payload: unknown): string | undefined {
  if (!isRecord(payload)) {
    return undefined;
  }

  const code = payload["code"];
  return typeof code === "string" ? code : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isMatchingRoute(
  response: Response,
  method: "GET" | "POST",
  route: string,
): boolean {
  if (response.request().method() !== method) {
    return false;
  }

  try {
    return new URL(response.url()).pathname === route;
  } catch {
    return response.url().endsWith(route);
  }
}

function waitForRouteResponse(
  page: Page,
  method: "GET" | "POST",
  route: string,
): Promise<Response> {
  return page.waitForResponse(
    (response) => isMatchingRoute(response, method, route),
    { timeout: 30_000 },
  );
}
