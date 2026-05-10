"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

import {
  checkOrganizationPermissionApi,
  createOrganizationApi,
  listOrganizationsApi,
  useOrganizationOperationState,
  type OrganizationAdminPermission,
  type OrganizationProofLink,
  type OrganizationSourceLink,
} from "@/lib/organization-api";
import {
  buildCheckOrganizationPermissionApiRequest,
  buildConfigurePermissionApiRequest,
  buildCreateOrganizationApiRequest,
  isOrganizationAdminPermission,
  ORGANIZATION_BEHAVIOR_FEATURE_PATH,
  ORGANIZATION_CREATE_BEHAVIOR_ID,
  ORGANIZATION_WEB_HARNESS_ROUTE,
  ORGANIZATION_WIRE_TEST_IDS,
  normalizeOrganizationName,
  organizationIdTestId,
  organizationInitialProjectsTestId,
  organizationPermissionsTestId,
  organizationRowTestId,
} from "@/lib/organization-routes";

const ORGANIZATION_PERMISSION_CACHE_KEY = "tanren.organization.permissions";

interface OrganizationRecord {
  id: string;
  name: string;
  grantedPermissions: OrganizationAdminPermission[];
  initialProjectCount: number | null;
  proofLink: OrganizationProofLink | null;
  sourceLink: OrganizationSourceLink | null;
}

export default function OrganizationsRoute(): ReactNode {
  return (
    <main className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 p-8">
      <header className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold">Organizations</h1>
        <p className="text-sm text-[--color-fg-muted]">
          The {ORGANIZATION_CREATE_BEHAVIOR_ID} behavior-proof witness surface
          is harness-owned at <code>{ORGANIZATION_WEB_HARNESS_ROUTE}</code>.
        </p>
        <p className="text-sm text-[--color-fg-muted]">
          This runtime route remains product-facing and does not mirror
          harness-side browser state.
        </p>
        <p className="text-sm text-[--color-fg-muted]">
          Shared feature source:{" "}
          <code>{ORGANIZATION_BEHAVIOR_FEATURE_PATH}</code>
        </p>
        <p>
          <Link className="underline" href={ORGANIZATION_WEB_HARNESS_ROUTE}>
            Open {ORGANIZATION_CREATE_BEHAVIOR_ID} harness witness surface
          </Link>
        </p>
      </header>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">Runtime route scope</h2>
        <p className="text-sm text-[--color-fg-muted]">
          Product-facing organization experiences stay on this route. Behavior
          proof witnesses are exercised through harness-owned routes only.
        </p>
      </section>
    </main>
  );
}

export function OrganizationHarnessRoute(): ReactNode {
  const [createName, setCreateName] = useState("");
  const [createIdempotencyKey, setCreateIdempotencyKey] = useState("");
  const [permissionOrgId, setPermissionOrgId] = useState("");
  const [permission, setPermission] = useState<
    OrganizationAdminPermission | ""
  >("");
  const [permissionOptions, setPermissionOptions] = useState<
    OrganizationAdminPermission[]
  >([]);
  const [organizationsByName, setOrganizationsByName] = useState<
    Record<string, OrganizationRecord>
  >({});
  const {
    operation,
    operationSequence,
    beginOperation,
    succeedOperation,
    failOperation,
  } = useOrganizationOperationState();

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
        (entry): entry is OrganizationAdminPermission =>
          isOrganizationAdminPermission(entry),
      );
      if (options.length > 0) {
        setPermissionOptions(options);
        setPermission((previous) => previous || options[0] || "");
      }
    } catch {
      // Ignore malformed local-storage state and keep runtime defaults.
    }
  }, []);

  async function createOrganization(): Promise<void> {
    beginOperation();

    const response = await createOrganizationApi(
      buildCreateOrganizationApiRequest(createName, createIdempotencyKey),
    );

    if (!response.ok) {
      failOperation(response);
      return;
    }

    const normalized = normalizeOrganizationName(
      response.body.organization.name,
    );

    setOrganizationsByName((previous) => ({
      ...previous,
      [normalized]: {
        id: response.body.organization.id,
        name: response.body.organization.name,
        grantedPermissions: response.body.granted_permissions,
        initialProjectCount: response.body.initial_project_count,
        proofLink: response.body.proof_link,
        sourceLink: response.body.source_link,
      },
    }));

    if (response.body.available_permissions.length > 0) {
      setPermissionOptions(response.body.available_permissions);
      window.localStorage.setItem(
        ORGANIZATION_PERMISSION_CACHE_KEY,
        JSON.stringify(response.body.available_permissions),
      );
      setPermission(
        (previous) => previous || response.body.available_permissions[0] || "",
      );
    }

    setPermissionOrgId(response.body.organization.id);
    succeedOperation();
  }

  async function listOrganizations(): Promise<void> {
    beginOperation();

    const response = await listOrganizationsApi();
    if (!response.ok) {
      failOperation(response);
      return;
    }

    setOrganizationsByName((previous) => {
      const next = { ...previous };
      for (const organization of response.body.organizations) {
        const key = normalizeOrganizationName(organization.name);
        const prior = previous[key];
        next[key] = {
          id: organization.id,
          name: organization.name,
          grantedPermissions: prior?.grantedPermissions ?? [],
          initialProjectCount: prior?.initialProjectCount ?? null,
          proofLink: prior?.proofLink ?? null,
          sourceLink: prior?.sourceLink ?? null,
        };
      }
      return next;
    });

    succeedOperation();
  }

  async function checkPermission(): Promise<void> {
    beginOperation();
    if (!isOrganizationAdminPermission(permission)) {
      failOperation({
        ok: false,
        status: 400,
        text: "permission is required",
        json: null,
        error: {
          code: "validation_failed",
          summary: "permission is required",
        },
        transportFallback: false,
      });
      return;
    }

    const request =
      permission === "configure"
        ? buildConfigurePermissionApiRequest(permissionOrgId)
        : buildCheckOrganizationPermissionApiRequest(
            permissionOrgId,
            permission,
          );
    const response = await checkOrganizationPermissionApi(request);

    if (!response.ok) {
      failOperation(response);
      return;
    }

    succeedOperation();
  }

  return (
    <main
      className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 p-8"
      data-testid={ORGANIZATION_WIRE_TEST_IDS.page}
    >
      <h1 className="text-2xl font-semibold">
        {ORGANIZATION_CREATE_BEHAVIOR_ID} Organization Harness Witness Surface
      </h1>
      <p className="text-sm text-[--color-fg-muted]">
        Harness-owned web witness for create/list/permission checks.
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
        <label
          className="mb-2 block text-sm"
          htmlFor="org-create-idempotency-key"
        >
          Idempotency key (optional)
        </label>
        <input
          id="org-create-idempotency-key"
          className="mb-3 w-full rounded border border-[--color-border] px-3 py-2"
          data-testid={ORGANIZATION_WIRE_TEST_IDS.createIdempotencyKeyInput}
          onChange={(event) => setCreateIdempotencyKey(event.target.value)}
          value={createIdempotencyKey}
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
          onChange={(event) => {
            const value = event.target.value;
            setPermission(isOrganizationAdminPermission(value) ? value : "");
          }}
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
              {organization.proofLink ? (
                <div className="text-xs text-[--color-fg-muted]">
                  proof: {organization.proofLink.behavior_id}
                </div>
              ) : null}
              {organization.sourceLink ? (
                <div className="text-xs text-[--color-fg-muted]">
                  source: {organization.sourceLink.event_family}/
                  {organization.sourceLink.event_kind}
                </div>
              ) : null}
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}
