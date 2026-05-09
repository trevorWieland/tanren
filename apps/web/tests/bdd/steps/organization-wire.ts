import type { Page, Response } from "@playwright/test";

import {
  ORGANIZATION_API_ROUTES,
  ORGANIZATION_WEB_ROUTE,
  ORGANIZATION_WIRE_TEST_IDS,
  normalizeOrganizationName,
  organizationIdTestId,
  organizationInitialProjectsTestId,
  organizationPermissionsTestId,
  organizationRowTestId,
} from "@/lib/organization-routes";

import { waitForHydration } from "./api-client";
import {
  assertOrganizationPermission,
  isCheckOrganizationPermissionResponse,
  isCreateOrganizationResponse,
  isListOrganizationsResponse,
  isOrganizationErrorResponse,
  type CheckOrganizationPermissionResponse,
  type CreateOrganizationResponse,
  type ListOrganizationsResponse,
  type OrganizationErrorResponse,
  type OrganizationSnapshot,
} from "./organization-world";

export interface WireOutcome {
  status: "idle" | "pending" | "success" | "failure";
  failureCode?: string;
  failureDetail?: string;
}

export interface DecodedWireSuccess<TBody> {
  ok: true;
  status: number;
  text: string;
  json: unknown;
  body: TBody;
}

export interface DecodedWireFailure {
  ok: false;
  status: number;
  text: string;
  json: unknown;
  error: OrganizationErrorResponse;
}

export type DecodedWireResponse<TBody> =
  | DecodedWireSuccess<TBody>
  | DecodedWireFailure;

export interface CreateOrganizationOperation {
  outcome: WireOutcome;
  response: DecodedWireResponse<CreateOrganizationResponse>;
  snapshot?: OrganizationSnapshot;
}

export interface ListOrganizationsOperation {
  outcome: WireOutcome;
  response: DecodedWireResponse<ListOrganizationsResponse>;
}

export interface CheckPermissionOperation {
  outcome: WireOutcome;
  response: DecodedWireResponse<CheckOrganizationPermissionResponse>;
}

export async function openOrganizationWireSurface(page: Page): Promise<void> {
  await page.goto(ORGANIZATION_WEB_ROUTE);
  await waitForHydration(page);
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
      if (!statusTarget || !sequenceTarget) {
        return false;
      }

      const status = (statusTarget.textContent ?? "").trim();
      const sequence = Number.parseInt(
        (sequenceTarget.textContent ?? "").trim(),
        10,
      );
      if (!Number.isFinite(sequence) || sequence <= previous) {
        return false;
      }

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

export async function readOrganizationSnapshot(
  page: Page,
  organizationName: string,
): Promise<OrganizationSnapshot> {
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

  const grantedPermissions =
    permissionsRaw === ""
      ? []
      : permissionsRaw
          .split(",")
          .map((permission) => assertOrganizationPermission(permission.trim()));

  const parsedProjectCount = Number.parseInt(projectRaw, 10);
  const initialProjectCount =
    projectRaw === "unknown" || Number.isNaN(parsedProjectCount)
      ? null
      : parsedProjectCount;

  return {
    id,
    name: organizationName,
    grantedPermissions,
    initialProjectCount,
  };
}

export async function createOrganizationViaWire(
  page: Page,
  organizationName: string,
): Promise<CreateOrganizationOperation> {
  await openOrganizationWireSurface(page);
  await page
    .getByTestId(ORGANIZATION_WIRE_TEST_IDS.createNameInput)
    .fill(organizationName);

  const previousSequence = await readWireSequence(page);
  const responsePromise = waitForOrganizationRouteResponse(
    page,
    "POST",
    ORGANIZATION_API_ROUTES.create,
  );

  await page.getByTestId(ORGANIZATION_WIRE_TEST_IDS.createSubmit).click();

  const [outcome, response] = await Promise.all([
    waitForWireOutcome(page, previousSequence),
    responsePromise,
  ]);

  const decodedResponse = await decodeWireResponse(
    response,
    isCreateOrganizationResponse,
    "create organization",
  );
  const snapshot =
    outcome.status === "success" && decodedResponse.ok
      ? await readOrganizationSnapshot(page, organizationName)
      : undefined;

  if (snapshot) {
    return {
      outcome,
      response: decodedResponse,
      snapshot,
    };
  }

  return {
    outcome,
    response: decodedResponse,
  };
}

export async function listOrganizationsViaWire(
  page: Page,
): Promise<ListOrganizationsOperation> {
  await openOrganizationWireSurface(page);

  const previousSequence = await readWireSequence(page);
  const responsePromise = waitForOrganizationRouteResponse(
    page,
    "GET",
    ORGANIZATION_API_ROUTES.list,
  );

  await page.getByTestId(ORGANIZATION_WIRE_TEST_IDS.listSubmit).click();

  const [outcome, response] = await Promise.all([
    waitForWireOutcome(page, previousSequence),
    responsePromise,
  ]);

  return {
    outcome,
    response: await decodeWireResponse(
      response,
      isListOrganizationsResponse,
      "list organizations",
    ),
  };
}

export async function checkOrganizationPermissionViaWire(
  page: Page,
  orgId: string,
  permission: string,
): Promise<CheckPermissionOperation> {
  await openOrganizationWireSurface(page);
  await page
    .getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionOrgIdInput)
    .fill(orgId);
  await page
    .getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionSelect)
    .selectOption(assertOrganizationPermission(permission));

  const previousSequence = await readWireSequence(page);
  const responsePromise = waitForOrganizationRouteResponse(
    page,
    "POST",
    ORGANIZATION_API_ROUTES.checkPermission,
  );

  await page.getByTestId(ORGANIZATION_WIRE_TEST_IDS.permissionSubmit).click();

  const [outcome, response] = await Promise.all([
    waitForWireOutcome(page, previousSequence),
    responsePromise,
  ]);

  return {
    outcome,
    response: await decodeWireResponse(
      response,
      isCheckOrganizationPermissionResponse,
      "check organization permission",
    ),
  };
}

export async function decodeWireResponse<TBody>(
  response: Response,
  isSuccessBody: (payload: unknown) => payload is TBody,
  operation: string,
): Promise<DecodedWireResponse<TBody>> {
  const text = await response.text();
  const json = tryParseJson(text);

  if (response.ok() && isSuccessBody(json)) {
    return {
      ok: true,
      status: response.status(),
      text,
      json,
      body: json,
    };
  }

  const fallback = fallbackErrorResponse(response.status(), operation, text);
  if (isOrganizationErrorResponse(json)) {
    return {
      ok: false,
      status: response.status(),
      text,
      json,
      error: {
        code: json.code,
        summary:
          typeof json.summary === "string" ? json.summary : fallback.summary,
      },
    };
  }

  return {
    ok: false,
    status: response.status(),
    text,
    json,
    error: fallback,
  };
}

function tryParseJson(text: string): unknown {
  if (text.trim() === "") {
    return null;
  }

  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

function fallbackErrorResponse(
  status: number,
  operation: string,
  text: string,
): OrganizationErrorResponse {
  if (status === 401) {
    return { code: "auth_required", summary: "authentication required" };
  }
  if (status === 403) {
    return { code: "permission_denied", summary: "permission denied" };
  }
  if (status === 400) {
    return {
      code: "validation_failed",
      summary: text || `${operation} invalid`,
    };
  }

  return {
    code: "unknown",
    summary: text || `${operation} failed with HTTP ${status}`,
  };
}

function isMatchingOrganizationRoute(
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

function waitForOrganizationRouteResponse(
  page: Page,
  method: "GET" | "POST",
  route: string,
): Promise<Response> {
  return page.waitForResponse(
    (response) => isMatchingOrganizationRoute(response, method, route),
    { timeout: 30_000 },
  );
}

export function organizationKey(name: string): string {
  return normalizeOrganizationName(name);
}
