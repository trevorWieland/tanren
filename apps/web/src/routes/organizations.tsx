"use client";

import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

import {
  ORGANIZATION_API_ROUTES,
  ORGANIZATION_WIRE_TEST_IDS,
  type OrganizationAdminPermission,
  organizationIdTestId,
  organizationInitialProjectsTestId,
  organizationPermissionsTestId,
  organizationRowTestId,
  normalizeOrganizationName,
} from "@/lib/organization-routes";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://127.0.0.1:8081";
const ORGANIZATION_PERMISSION_CACHE_KEY = "tanren.organization.permissions";

interface WireResponse {
  ok: boolean;
  status: number;
  text: string;
  json: unknown;
}

interface OrganizationRecord {
  id: string;
  name: string;
  grantedPermissions: string[];
  initialProjectCount: number | null;
}

interface OperationState {
  status: "idle" | "pending" | "success" | "failure";
  failureCode: string | null;
  detail: string | null;
}

interface EventEnvelope {
  payload?: unknown;
}

function failureCode(response: WireResponse): string {
  const body = response.json as { code?: unknown } | null;
  if (body && typeof body.code === "string") {
    return body.code;
  }
  if (response.status === 401) return "auth_required";
  if (response.status === 403) return "permission_denied";
  return "unknown";
}

function failureDetail(response: WireResponse): string {
  return `status=${response.status} code=${failureCode(response)} body=${response.text}`;
}

async function callApi(
  method: "GET" | "POST",
  path: string,
  payload?: unknown,
): Promise<WireResponse> {
  const headers: Record<string, string> = {};
  const request: RequestInit = {
    method,
    headers,
    credentials: "include",
  };
  if (payload !== undefined) {
    headers["content-type"] = "application/json";
    request.body = JSON.stringify(payload);
  }

  const response = await fetch(`${API_URL}${path}`, request);

  const text = await response.text();
  let json: unknown = null;
  try {
    json = JSON.parse(text);
  } catch {
    json = null;
  }

  return {
    ok: response.ok,
    status: response.status,
    text,
    json,
  };
}

async function readInitialProjectCount(
  organizationName: string,
): Promise<number | null> {
  const response = await callApi(
    "GET",
    `${ORGANIZATION_API_ROUTES.testHookEvents}?limit=200`,
  );
  if (!response.ok) {
    return null;
  }

  const events = response.json as EventEnvelope[];
  const created = [...events]
    .reverse()
    .map((event) => event.payload as Record<string, unknown> | undefined)
    .find((payload) => {
      if (!payload) return false;
      if (payload["kind"] !== "organization_created") return false;
      const eventPayload = payload["payload"] as
        | Record<string, unknown>
        | undefined;
      if (!eventPayload) return false;
      return eventPayload["name"] === organizationName;
    });

  if (!created) {
    return null;
  }

  const eventPayload = created["payload"] as
    | Record<string, unknown>
    | undefined;
  const initialProjectCount = eventPayload?.["initial_project_count"];
  return typeof initialProjectCount === "number" ? initialProjectCount : null;
}

export default function OrganizationsRoute(): ReactNode {
  const [createName, setCreateName] = useState("");
  const [permissionOrgId, setPermissionOrgId] = useState("");
  const [operationSequence, setOperationSequence] = useState(0);
  const [permission, setPermission] = useState<OrganizationAdminPermission>("");
  const [permissionOptions, setPermissionOptions] = useState<
    OrganizationAdminPermission[]
  >([]);
  const [organizationsByName, setOrganizationsByName] = useState<
    Record<string, OrganizationRecord>
  >({});
  const [operation, setOperation] = useState<OperationState>({
    status: "idle",
    failureCode: null,
    detail: null,
  });

  const organizations = useMemo(
    () =>
      Object.values(organizationsByName).sort((a, b) =>
        a.name.localeCompare(b.name),
      ),
    [organizationsByName],
  );
  useEffect(() => {
    const cached = window.localStorage.getItem(
      ORGANIZATION_PERMISSION_CACHE_KEY,
    );
    if (!cached) {
      return;
    }
    try {
      const parsed = JSON.parse(cached) as unknown;
      if (!Array.isArray(parsed)) {
        return;
      }
      const options = parsed.filter(
        (entry): entry is string => typeof entry === "string",
      );
      if (options.length > 0) {
        setPermissionOptions(options);
        setPermission((previous) => previous || options[0] || "");
      }
    } catch {
      // Ignore malformed local-storage state and keep runtime defaults.
    }
  }, []);

  function beginOperation(): void {
    setOperationSequence((previous) => previous + 1);
    setOperation({ status: "pending", failureCode: null, detail: null });
  }

  async function createOrganization(): Promise<void> {
    beginOperation();
    const response = await callApi("POST", ORGANIZATION_API_ROUTES.create, {
      name: createName,
    });

    if (!response.ok) {
      setOperation({
        status: "failure",
        failureCode: failureCode(response),
        detail: failureDetail(response),
      });
      return;
    }

    const body = response.json as {
      organization: { id: string; name: string };
      granted_permissions: string[];
    };
    const initialProjectCount = await readInitialProjectCount(
      body.organization.name,
    );
    const normalized = normalizeOrganizationName(body.organization.name);

    setOrganizationsByName((previous) => ({
      ...previous,
      [normalized]: {
        id: body.organization.id,
        name: body.organization.name,
        grantedPermissions: body.granted_permissions,
        initialProjectCount,
      },
    }));
    if (body.granted_permissions.length > 0) {
      setPermissionOptions(body.granted_permissions);
      window.localStorage.setItem(
        ORGANIZATION_PERMISSION_CACHE_KEY,
        JSON.stringify(body.granted_permissions),
      );
      setPermission(
        (previous) => previous || body.granted_permissions[0] || "",
      );
    }
    setPermissionOrgId(body.organization.id);
    setOperation({ status: "success", failureCode: null, detail: null });
  }

  async function listOrganizations(): Promise<void> {
    beginOperation();
    const response = await callApi("GET", ORGANIZATION_API_ROUTES.list);

    if (!response.ok) {
      setOperation({
        status: "failure",
        failureCode: failureCode(response),
        detail: failureDetail(response),
      });
      return;
    }

    const body = response.json as {
      organizations: Array<{ id: string; name: string }>;
    };

    setOrganizationsByName((previous) => {
      const next = { ...previous };
      for (const organization of body.organizations) {
        const key = normalizeOrganizationName(organization.name);
        const prior = previous[key];
        next[key] = {
          id: organization.id,
          name: organization.name,
          grantedPermissions: prior?.grantedPermissions ?? [],
          initialProjectCount: prior?.initialProjectCount ?? null,
        };
      }
      return next;
    });

    setOperation({ status: "success", failureCode: null, detail: null });
  }

  async function checkPermission(): Promise<void> {
    beginOperation();
    const response = await callApi(
      "POST",
      ORGANIZATION_API_ROUTES.checkPermission,
      {
        org_id: permissionOrgId,
        permission,
      },
    );

    if (!response.ok) {
      setOperation({
        status: "failure",
        failureCode: failureCode(response),
        detail: failureDetail(response),
      });
      return;
    }

    setOperation({ status: "success", failureCode: null, detail: null });
  }

  return (
    <main
      className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 p-8"
      data-testid={ORGANIZATION_WIRE_TEST_IDS.page}
    >
      <h1 className="text-2xl font-semibold">Organization Wire Surface</h1>
      <p className="text-sm text-[--color-fg-muted]">
        Web-owned harness for B-0066 organization create/list/permission
        witness.
      </p>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">Create organization</h2>
        <label className="mb-2 block text-sm" htmlFor="org-create-name">
          Organization name
        </label>
        <input
          id="org-create-name"
          className="mb-3 w-full rounded border border-[--color-border] px-3 py-2"
          data-testid={ORGANIZATION_WIRE_TEST_IDS.createNameInput}
          onChange={(event) => setCreateName(event.target.value)}
          value={createName}
        />
        <button
          className="rounded border border-[--color-border] px-3 py-2"
          data-testid={ORGANIZATION_WIRE_TEST_IDS.createSubmit}
          onClick={() => {
            void createOrganization();
          }}
          type="button"
        >
          Create organization
        </button>
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">List organizations</h2>
        <button
          className="rounded border border-[--color-border] px-3 py-2"
          data-testid={ORGANIZATION_WIRE_TEST_IDS.listSubmit}
          onClick={() => {
            void listOrganizations();
          }}
          type="button"
        >
          List available organizations
        </button>
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">Permission check</h2>
        <label className="mb-2 block text-sm" htmlFor="org-permission-id">
          Organization id
        </label>
        <input
          id="org-permission-id"
          className="mb-3 w-full rounded border border-[--color-border] px-3 py-2"
          data-testid={ORGANIZATION_WIRE_TEST_IDS.permissionOrgIdInput}
          onChange={(event) => setPermissionOrgId(event.target.value)}
          value={permissionOrgId}
        />
        <label className="mb-2 block text-sm" htmlFor="org-permission-select">
          Permission
        </label>
        <select
          className="mb-3 w-full rounded border border-[--color-border] px-3 py-2"
          data-testid={ORGANIZATION_WIRE_TEST_IDS.permissionSelect}
          id="org-permission-select"
          onChange={(event) =>
            setPermission(event.target.value as OrganizationAdminPermission)
          }
          value={permission}
        >
          {permissionOptions.map((candidate) => (
            <option key={candidate} value={candidate}>
              {candidate}
            </option>
          ))}
        </select>
        <button
          className="rounded border border-[--color-border] px-3 py-2"
          data-testid={ORGANIZATION_WIRE_TEST_IDS.permissionSubmit}
          onClick={() => {
            void checkPermission();
          }}
          type="button"
        >
          Check permission
        </button>
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 font-mono text-sm">
        <h2 className="mb-2 text-lg font-medium">Last operation</h2>
        <p data-testid={ORGANIZATION_WIRE_TEST_IDS.operationStatus}>
          {operation.status}
        </p>
        <p data-testid={ORGANIZATION_WIRE_TEST_IDS.operationSequence}>
          {operationSequence}
        </p>
        {operation.failureCode ? (
          <p data-testid={ORGANIZATION_WIRE_TEST_IDS.failureCode}>
            {operation.failureCode}
          </p>
        ) : null}
        {operation.detail ? (
          <p data-testid={ORGANIZATION_WIRE_TEST_IDS.failureDetail}>
            {operation.detail}
          </p>
        ) : null}
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">Observed organizations</h2>
        <ul
          className="flex flex-col gap-2 font-mono text-sm"
          data-testid={ORGANIZATION_WIRE_TEST_IDS.organizationsList}
        >
          {organizations.map((organization) => (
            <li
              className="rounded border border-[--color-border] p-2"
              data-testid={organizationRowTestId(organization.name)}
              key={organization.id}
            >
              <div>{organization.name}</div>
              <div data-testid={organizationIdTestId(organization.name)}>
                {organization.id}
              </div>
              <div
                data-testid={organizationInitialProjectsTestId(
                  organization.name,
                )}
              >
                {organization.initialProjectCount === null
                  ? "unknown"
                  : String(organization.initialProjectCount)}
              </div>
              <div
                data-testid={organizationPermissionsTestId(organization.name)}
              >
                {organization.grantedPermissions.join(",")}
              </div>
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}
