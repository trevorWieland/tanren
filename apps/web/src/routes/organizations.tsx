"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

import {
  checkOrganizationPermissionApi,
  createOrganizationApi,
  listOrganizationsApi,
  useOrganizationOperationState,
  type OrganizationAdminPermission,
  type OrganizationCapabilityView,
  type OrganizationEventReference,
  type OrganizationProofLink,
  type ReadModelFreshness,
  type OrganizationSourceLink,
} from "@/lib/organization-api";
import {
  buildCheckOrganizationPermissionApiRequest,
  buildConfigurePermissionApiRequest,
  buildCreateOrganizationApiRequest,
  isOrganizationAdminPermission,
  ORGANIZATION_PRODUCT_TEST_IDS,
  ORGANIZATION_WEB_SURFACE_CONTRACT,
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
  capabilities: OrganizationCapabilityView[];
  initialProjectCount: number | null;
  proofLink: OrganizationProofLink | null;
  sourceLink: OrganizationSourceLink | null;
  sourceEvent: OrganizationEventReference | null;
}

type AuthGateStatus = "checking" | "unauthenticated" | "authenticated";

export default function OrganizationsRoute(): ReactNode {
  const router = useRouter();
  const [authGate, setAuthGate] = useState<AuthGateStatus>("checking");
  const [organizations, setOrganizations] = useState<OrganizationRecord[]>([]);
  const [freshness, setFreshness] = useState<ReadModelFreshness | null>(null);
  const [sourceLink, setSourceLink] = useState<OrganizationSourceLink | null>(
    null,
  );
  const [nextCursor, setNextCursor] = useState<string | null>(null);

  const fetchOrganizations = useCallback(async (): Promise<void> => {
    const response = await listOrganizationsApi();
    if (!response.ok && response.status === 401) {
      setAuthGate("unauthenticated");
      return;
    }
    if (!response.ok) {
      return;
    }
    setAuthGate("authenticated");
    setOrganizations(
      response.body.organizations.map((org) => ({
        id: org.id,
        name: org.name,
        grantedPermissions: org.capabilities
          .filter((cap) => cap.allowed)
          .map((cap) => cap.permission),
        capabilities: org.capabilities,
        initialProjectCount: null,
        proofLink: null,
        sourceLink: null,
        sourceEvent: null,
      })),
    );
    setFreshness(response.body.freshness);
    setSourceLink(response.body.source_link);
    setNextCursor(response.body.next_cursor ?? null);
  }, []);

  useEffect(() => {
    void fetchOrganizations();
  }, [fetchOrganizations]);

  useEffect(() => {
    if (authGate !== "unauthenticated") {
      return;
    }
    router.push("/sign-in");
  }, [authGate, router]);

  if (authGate === "checking") {
    return (
      <main
        className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 p-8"
        data-testid={ORGANIZATION_PRODUCT_TEST_IDS.authGate}
      >
        <header className="flex flex-col gap-2">
          <h1 className="text-2xl font-semibold">Organizations</h1>
          <p className="text-sm text-[--color-fg-muted]">
            Resolving actor context…
          </p>
        </header>
      </main>
    );
  }

  return (
    <main
      className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 p-8"
      data-testid={ORGANIZATION_PRODUCT_TEST_IDS.authGate}
    >
      <header className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold">Organizations</h1>
        <p className="text-sm text-[--color-fg-muted]">
          The {ORGANIZATION_WEB_SURFACE_CONTRACT.behaviorId} behavior-proof
          witness surface is harness-owned at{" "}
          <code>{ORGANIZATION_WEB_SURFACE_CONTRACT.harnessRoute}</code>.
        </p>
        <p className="text-sm text-[--color-fg-muted]">
          Shared feature source:{" "}
          <code>{ORGANIZATION_WEB_SURFACE_CONTRACT.featurePath}</code>
        </p>
        <p>
          <Link
            className="underline"
            href={ORGANIZATION_WEB_SURFACE_CONTRACT.harnessRoute}
          >
            Open {ORGANIZATION_WEB_SURFACE_CONTRACT.behaviorId} harness witness
            surface
          </Link>
        </p>
      </header>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">Your organizations</h2>
        <ul
          className="flex flex-col gap-2 font-mono text-sm"
          data-testid={ORGANIZATION_PRODUCT_TEST_IDS.organizationsList}
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
                data-testid={organizationPermissionsTestId(organization.name)}
              >
                {organization.grantedPermissions.join(",")}
              </div>
              <div className="text-xs text-[--color-fg-muted]">
                capabilities:{" "}
                {organization.capabilities
                  .map(
                    (capability) =>
                      `${capability.permission}:${capability.allowed}`,
                  )
                  .join(",")}
              </div>
            </li>
          ))}
        </ul>
        {organizations.length === 0 ? (
          <p
            className="text-sm text-[--color-fg-muted]"
            data-testid={ORGANIZATION_PRODUCT_TEST_IDS.emptyState}
          >
            No organizations yet.
          </p>
        ) : null}
      </section>

      {sourceLink ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 font-mono text-sm">
          <h2 className="mb-2 text-lg font-medium">Proof and source</h2>
          <p
            className="text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_PRODUCT_TEST_IDS.sourceLink}
          >
            source: {sourceLink.event_family}/{sourceLink.event_kind}
          </p>
        </section>
      ) : null}

      {freshness ? (
        <p
          className="text-xs text-[--color-fg-muted]"
          data-testid={ORGANIZATION_PRODUCT_TEST_IDS.freshness}
        >
          freshness: projection={freshness.projection} generated_at=
          {freshness.generated_at} cursor={freshness.cursor ?? "<none>"}{" "}
          checkpoint={freshness.checkpoint ?? "<none>"}
        </p>
      ) : null}

      {nextCursor !== null ? (
        <p
          className="text-xs text-[--color-fg-muted]"
          data-testid={ORGANIZATION_PRODUCT_TEST_IDS.listNextCursor}
        >
          next_cursor: {nextCursor}
        </p>
      ) : null}
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
  const [listFreshness, setListFreshness] = useState<ReadModelFreshness | null>(
    null,
  );
  const [listNextCursor, setListNextCursor] = useState<string | null>(null);
  const [listSourceLink, setListSourceLink] =
    useState<OrganizationSourceLink | null>(null);
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
        grantedPermissions: response.body.granted_permissions.filter(
          (p): p is OrganizationAdminPermission =>
            isOrganizationAdminPermission(p),
        ),
        capabilities: response.body.organization.capabilities,
        initialProjectCount: response.body.initial_project_count,
        proofLink: response.body.proof_link,
        sourceLink: response.body.source_link,
        sourceEvent: response.body.source_event ?? null,
      },
    }));

    if (
      response.body.available_permissions.length > 0 &&
      permissionOptions.length === 0
    ) {
      const adminPermissions = response.body.available_permissions.filter(
        (p): p is OrganizationAdminPermission =>
          isOrganizationAdminPermission(p),
      );
      setPermissionOptions(adminPermissions);
      setPermission((previous) => previous || adminPermissions[0] || "");
      window.localStorage.setItem(
        ORGANIZATION_PERMISSION_CACHE_KEY,
        JSON.stringify(adminPermissions),
      );
    }

    setListFreshness(null);
    setListNextCursor(null);
    setListSourceLink(null);
    succeedOperation();
  }

  async function listOrganizations(): Promise<void> {
    beginOperation();
    const response = await listOrganizationsApi();

    if (!response.ok) {
      failOperation(response);
      return;
    }

    const mapped: Record<string, OrganizationRecord> = {};
    for (const org of response.body.organizations) {
      const normalized = normalizeOrganizationName(org.name);
      mapped[normalized] = {
        id: org.id,
        name: org.name,
        grantedPermissions: org.capabilities
          .filter((cap) => cap.allowed)
          .map((cap) => cap.permission),
        capabilities: org.capabilities,
        initialProjectCount: null,
        proofLink: null,
        sourceLink: null,
        sourceEvent: null,
      };
    }

    setOrganizationsByName((previous) => ({ ...previous, ...mapped }));
    setListFreshness(response.body.freshness);
    setListNextCursor(response.body.next_cursor ?? null);
    setListSourceLink(response.body.source_link);
    succeedOperation();
  }

  async function listOrganizationsNextPage(): Promise<void> {
    if (!listNextCursor) {
      return;
    }
    beginOperation();
    const response = await listOrganizationsApi({ cursor: listNextCursor });

    if (!response.ok) {
      failOperation(response);
      return;
    }

    const mapped: Record<string, OrganizationRecord> = {};
    for (const org of response.body.organizations) {
      const normalized = normalizeOrganizationName(org.name);
      mapped[normalized] = {
        id: org.id,
        name: org.name,
        grantedPermissions: org.capabilities
          .filter((cap) => cap.allowed)
          .map((cap) => cap.permission),
        capabilities: org.capabilities,
        initialProjectCount: null,
        proofLink: null,
        sourceLink: null,
        sourceEvent: null,
      };
    }

    setOrganizationsByName((previous) => ({ ...previous, ...mapped }));
    setListFreshness(response.body.freshness);
    setListNextCursor(response.body.next_cursor ?? null);
    setListSourceLink(response.body.source_link);
    succeedOperation();
  }

  async function checkPermission(): Promise<void> {
    if (!permission || !permissionOrgId) {
      return;
    }
    beginOperation();
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
    <main className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 p-8">
      <header className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold">
          B-0066 Organization wire harness
        </h1>
        <p className="text-sm text-[--color-fg-muted]">
          Shared feature source:{" "}
          <code>{ORGANIZATION_WEB_SURFACE_CONTRACT.featurePath}</code>
        </p>
      </header>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">Create organization</h2>
        <label className="mb-2 block text-sm" htmlFor="org-create-name">
          Name
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
          Idempotency key
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
          Create
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
          Refresh
        </button>
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">
          Check organization permission
        </h2>
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
              <div className="text-xs text-[--color-fg-muted]">
                capabilities:{" "}
                {organization.capabilities
                  .map(
                    (capability) =>
                      `${capability.permission}:${capability.allowed}`,
                  )
                  .join(",")}
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
              {organization.sourceEvent ? (
                <div className="text-xs text-[--color-fg-muted]">
                  source_event: {organization.sourceEvent.event_id}
                </div>
              ) : null}
            </li>
          ))}
        </ul>
        {listFreshness ? (
          <p
            className="mt-3 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_WIRE_TEST_IDS.listFreshness}
          >
            freshness: projection={listFreshness.projection} generated_at=
            {listFreshness.generated_at} cursor=
            {listFreshness.cursor ?? "<none>"} checkpoint=
            {listFreshness.checkpoint ?? "<none>"}
          </p>
        ) : null}
        {listNextCursor !== null ? (
          <p
            className="mt-1 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_WIRE_TEST_IDS.listNextCursor}
          >
            next_cursor: {listNextCursor}
          </p>
        ) : null}
        {listNextCursor !== null ? (
          <button
            className="mt-1 rounded border border-[--color-border] px-3 py-2 text-xs"
            data-testid={ORGANIZATION_WIRE_TEST_IDS.listNextPage}
            onClick={() => {
              void listOrganizationsNextPage();
            }}
            type="button"
          >
            Next page
          </button>
        ) : null}
        {listSourceLink ? (
          <p
            className="mt-1 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_PRODUCT_TEST_IDS.sourceLink}
          >
            source: {listSourceLink.event_family}/{listSourceLink.event_kind}
          </p>
        ) : null}
      </section>
    </main>
  );
}
