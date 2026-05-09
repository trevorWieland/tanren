"use client";

import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

import {
  checkOrganizationPermissionApi,
  createOrganizationApi,
  isOrganizationPermission,
  listOrganizationsApi,
  useOrganizationOperationState,
  type OrganizationAdminPermission,
  type OrganizationProofLink,
  type OrganizationSourceLink,
} from "@/lib/organization-api";
import {
  ORGANIZATION_WIRE_TEST_IDS,
  organizationIdTestId,
  organizationInitialProjectsTestId,
  organizationPermissionsTestId,
  organizationRowTestId,
  normalizeOrganizationName,
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
  const [createName, setCreateName] = useState("");
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
          isOrganizationPermission(entry),
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

    const response = await createOrganizationApi({
      name: createName,
    });

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

    if (response.body.granted_permissions.length > 0) {
      setPermissionOptions(response.body.granted_permissions);
      window.localStorage.setItem(
        ORGANIZATION_PERMISSION_CACHE_KEY,
        JSON.stringify(response.body.granted_permissions),
      );
      setPermission(
        (previous) => previous || response.body.granted_permissions[0] || "",
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
    if (!isOrganizationPermission(permission)) {
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

    const response = await checkOrganizationPermissionApi({
      org_id: permissionOrgId,
      permission,
    });

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
          onChange={(event) => {
            const value = event.target.value;
            setPermission(isOrganizationPermission(value) ? value : "");
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
