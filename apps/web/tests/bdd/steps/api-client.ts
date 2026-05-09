import type { Page } from "@playwright/test";

import {
  ORGANIZATION_WEB_ROUTE,
  ORGANIZATION_WIRE_TEST_IDS,
  organizationIdTestId,
  organizationInitialProjectsTestId,
  organizationPermissionsTestId,
  organizationRowTestId,
} from "@/lib/organization-routes";

import { actor, type OrganizationWorld } from "./organization-world";

interface WireOutcome {
  status: "idle" | "pending" | "success" | "failure";
  failureCode?: string;
  failureDetail?: string;
}

interface OrganizationWireSnapshot {
  id: string;
  grantedPermissions: string[];
  initialProjectCount: number | null;
}

export async function openOrganizationWireSurface(page: Page): Promise<void> {
  await page.goto(ORGANIZATION_WEB_ROUTE);
  await waitForHydration(page);
}

export async function waitForWireOutcome(
  page: Page,
  previousSequence: number,
): Promise<WireOutcome> {
  const statusLocator = page.getByTestId(
    ORGANIZATION_WIRE_TEST_IDS.operationStatus,
  );
  await page.waitForFunction(
    ({ statusTestId, sequenceTestId, previous }) => {
      const statusTarget = document.querySelector(
        `[data-testid="${statusTestId}"]`,
      );
      const sequenceTarget = document.querySelector(
        `[data-testid="${sequenceTestId}"]`,
      );
      if (!statusTarget || !sequenceTarget) return false;
      const status = (statusTarget.textContent ?? "").trim();
      const sequence = Number.parseInt(
        (sequenceTarget.textContent ?? "").trim(),
        10,
      );
      if (!Number.isFinite(sequence) || sequence <= previous) return false;
      return status === "success" || status === "failure";
    },
    {
      statusTestId: ORGANIZATION_WIRE_TEST_IDS.operationStatus,
      sequenceTestId: ORGANIZATION_WIRE_TEST_IDS.operationSequence,
      previous: previousSequence,
    },
    { timeout: 30_000 },
  );

  const statusText = (await statusLocator.innerText()).trim();
  const status =
    statusText === "success" || statusText === "failure" ? statusText : "idle";

  if (status === "failure") {
    const failureCode = await page
      .getByTestId(ORGANIZATION_WIRE_TEST_IDS.failureCode)
      .innerText();
    const failureDetail = await page
      .getByTestId(ORGANIZATION_WIRE_TEST_IDS.failureDetail)
      .innerText();
    return {
      status,
      failureCode: failureCode.trim(),
      failureDetail: failureDetail.trim(),
    };
  }

  return { status };
}

export async function readWireSequence(page: Page): Promise<number> {
  const text = (
    await page
      .getByTestId(ORGANIZATION_WIRE_TEST_IDS.operationSequence)
      .innerText()
  ).trim();
  const sequence = Number.parseInt(text, 10);
  return Number.isNaN(sequence) ? -1 : sequence;
}

export async function readOrganizationSnapshot(
  page: Page,
  organizationName: string,
): Promise<OrganizationWireSnapshot> {
  const row = page.getByTestId(organizationRowTestId(organizationName));
  await row.waitFor({ state: "visible", timeout: 30_000 });

  const id = (
    await page.getByTestId(organizationIdTestId(organizationName)).innerText()
  ).trim();
  const permissionsRaw = (
    await page
      .getByTestId(organizationPermissionsTestId(organizationName))
      .innerText()
  ).trim();
  const projectRaw = (
    await page
      .getByTestId(organizationInitialProjectsTestId(organizationName))
      .innerText()
  ).trim();

  return {
    id,
    grantedPermissions:
      permissionsRaw === ""
        ? []
        : permissionsRaw.split(",").map((permission) => permission.trim()),
    initialProjectCount:
      projectRaw === "unknown" ? null : Number.parseInt(projectRaw, 10),
  };
}

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
  await page.getByRole("button", { name: /^sign in$/i }).click();
  const result = await Promise.race([
    page.waitForURL("/").then(() => "ok" as const),
    page
      .locator('form [role="alert"]')
      .first()
      .waitFor({ state: "visible" })
      .then(() => "alert" as const),
  ]);
  if (result !== "ok") {
    a.hasSession = false;
    a.lastFailureCode = await classifyFailureFromAlert(page);
    throw new Error(`sign-in failed for ${name} via web UI`);
  }
  a.hasSession = true;
  delete a.lastFailureCode;
}

async function waitForHydration(page: Page): Promise<void> {
  await page.waitForFunction(
    () => {
      const root = document as unknown as Record<string, unknown>;
      const keys = Object.keys(root).filter(
        (key) =>
          key.startsWith("__reactContainer") ||
          key.startsWith("_reactRootContainer"),
      );
      if (keys.length > 0) return true;
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
  if (text.includes("invitation") && text.includes("expired"))
    return "invitation_expired";
  if (text.includes("invitation") && text.includes("not recognized"))
    return "invitation_not_found";
  if (text.includes("invitation") && text.includes("already been accepted"))
    return "invitation_already_consumed";
  if (text.includes("account already exists")) return "duplicate_identifier";
  if (text.includes("email or password is invalid"))
    return "invalid_credential";
  if (text.includes("check the form fields") || text.includes("required"))
    return "validation_failed";
  return "unknown";
}
