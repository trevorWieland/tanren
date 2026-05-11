"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";

import {
  listOrganizationMembersApi,
  type OrganizationMemberPermissionGrant,
  type OrganizationMemberView,
  type ReadModelFreshness,
  type OrganizationProofLink,
  type OrganizationSourceLink,
} from "@/lib/organization-api";
import {
  buildListOrganizationMembersApiRequest,
  ORGANIZATION_MEMBERS_BEHAVIOR_FEATURE_PATH,
  ORGANIZATION_MEMBERS_BEHAVIOR_ID,
  ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS,
  ORGANIZATION_MEMBERS_WEB_HARNESS_ROUTE,
  ORGANIZATION_MEMBERS_WIRE_TEST_IDS,
  memberGrantSourceTestId,
  memberJoinedAtTestId,
  memberPermissionsTestId,
  memberRowTestId,
} from "@/lib/organization-routes";

const DEFAULT_MEMBERS_LIST_LIMIT = 50;

interface MemberRecord {
  accountId: string;
  identifier: string;
  joinedAt: string;
  grantedPermissions: OrganizationMemberPermissionGrant[];
}

type AuthGateStatus = "checking" | "unauthenticated" | "authenticated";

export function OrganizationMembersHarnessRoute(): ReactNode {
  return <OrganizationMembersSurface />;
}

export default function OrganizationMembersRoute(): ReactNode {
  return <OrganizationMembersSurface />;
}

function OrganizationMembersSurface(): ReactNode {
  const router = useRouter();
  const params = useParams<{ orgId: string }>();
  const orgId = params?.orgId ?? "";
  const [authGate, setAuthGate] = useState<AuthGateStatus>("checking");
  const [members, setMembers] = useState<MemberRecord[]>([]);
  const [freshness, setFreshness] = useState<ReadModelFreshness | null>(null);
  const [proofLink, setProofLink] = useState<OrganizationProofLink | null>(
    null,
  );
  const [sourceLink, setSourceLink] = useState<OrganizationSourceLink | null>(
    null,
  );
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [currentCursor, setCurrentCursor] = useState<string | null>(null);

  const fetchMembers = useCallback(
    async (cursor?: string | null): Promise<void> => {
      const request = buildListOrganizationMembersApiRequest({
        limit: DEFAULT_MEMBERS_LIST_LIMIT,
        cursor: cursor ?? null,
      });
      const response = await listOrganizationMembersApi(orgId, request);
      if (!response.ok && response.status === 401) {
        setAuthGate("unauthenticated");
        return;
      }
      if (!response.ok) {
        return;
      }
      setAuthGate("authenticated");
      setMembers(
        response.body.members.map(
          (member: OrganizationMemberView): MemberRecord => ({
            accountId: member.account_id,
            identifier: member.identifier,
            joinedAt: member.joined_at,
            grantedPermissions: member.granted_permissions,
          }),
        ),
      );
      setFreshness(response.body.freshness);
      setProofLink(response.body.proof_link);
      setSourceLink(response.body.source_link);
      setNextCursor(response.body.next_cursor ?? null);
    },
    [orgId],
  );

  useEffect(() => {
    void fetchMembers(currentCursor);
  }, [currentCursor, fetchMembers]);

  useEffect(() => {
    if (authGate !== "unauthenticated") {
      return;
    }
    router.push("/sign-in");
  }, [authGate, router]);

  const listNextPage = useCallback(async (): Promise<void> => {
    setCurrentCursor(nextCursor);
  }, [nextCursor]);

  if (authGate === "checking") {
    return (
      <main
        className="mx-auto flex min-h-screen w-full max-w-3xl flex-col gap-6 p-8"
        data-testid={ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS.authGate}
      >
        <header className="flex flex-col gap-2">
          <h1 className="text-2xl font-semibold">Organization Members</h1>
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
      data-testid={ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS.authGate}
    >
      <header className="flex flex-col gap-2">
        <h1 className="text-2xl font-semibold">Organization Members</h1>
        <p className="text-sm text-[--color-fg-muted]">
          The {ORGANIZATION_MEMBERS_BEHAVIOR_ID} behavior-proof witness surface
          is harness-owned at{" "}
          <code>{ORGANIZATION_MEMBERS_WEB_HARNESS_ROUTE}</code>.
        </p>
        <p className="text-sm text-[--color-fg-muted]">
          Shared feature source:{" "}
          <code>{ORGANIZATION_MEMBERS_BEHAVIOR_FEATURE_PATH}</code>
        </p>
        <p>
          <Link
            className="underline"
            href={ORGANIZATION_MEMBERS_WEB_HARNESS_ROUTE}
          >
            Open {ORGANIZATION_MEMBERS_BEHAVIOR_ID} harness witness surface
          </Link>
        </p>
      </header>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
        <h2 className="mb-3 text-lg font-medium">Members</h2>
        {members.length === 0 ? (
          <p
            className="text-sm text-[--color-fg-muted]"
            data-testid={ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS.emptyState}
          >
            No members visible.
          </p>
        ) : (
          <ul
            className="flex flex-col gap-2 font-mono text-sm"
            data-testid={ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS.membersList}
          >
            {members.map((member) => (
              <li
                className="rounded border border-[--color-border] p-2"
                data-testid={memberRowTestId(member.accountId)}
                key={member.accountId}
              >
                <div className="font-semibold">{member.identifier}</div>
                <div data-testid={memberPermissionsTestId(member.accountId)}>
                  {member.grantedPermissions
                    .map(
                      (grant: OrganizationMemberPermissionGrant) =>
                        `${grant.permission}(${grant.grant_source})`,
                    )
                    .join(", ")}
                </div>
                <div
                  className="text-xs text-[--color-fg-muted]"
                  data-testid={memberGrantSourceTestId(member.accountId)}
                >
                  grant sources:{" "}
                  {[
                    ...new Set(
                      member.grantedPermissions.map(
                        (grant: OrganizationMemberPermissionGrant) =>
                          grant.grant_source,
                      ),
                    ),
                  ].join(", ")}
                </div>
                <div
                  className="text-xs text-[--color-fg-muted]"
                  data-testid={memberJoinedAtTestId(member.accountId)}
                >
                  joined: {member.joinedAt}
                </div>
              </li>
            ))}
          </ul>
        )}
        {freshness ? (
          <p
            className="mt-3 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS.freshness}
          >
            freshness: projection={freshness.projection} generated_at=
            {freshness.generated_at} cursor=
            {freshness.cursor ?? "<none>"} checkpoint=
            {freshness.checkpoint ?? "<none>"}
          </p>
        ) : null}
        {proofLink ? (
          <p
            className="mt-1 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS.proofLink}
          >
            proof: {proofLink.behavior_id}
          </p>
        ) : null}
        {sourceLink ? (
          <p
            className="mt-1 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS.sourceLink}
          >
            source: {sourceLink.event_family}/{sourceLink.event_kind}
          </p>
        ) : null}
        {nextCursor !== null ? (
          <p
            className="mt-1 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_MEMBERS_PRODUCT_TEST_IDS.listNextCursor}
          >
            next_cursor: {nextCursor}
          </p>
        ) : null}
      </section>

      <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 font-mono text-sm">
        <h2 className="mb-2 text-lg font-medium">Wire harness</h2>
        <button
          className="rounded border border-[--color-border] px-3 py-2"
          data-testid={ORGANIZATION_MEMBERS_WIRE_TEST_IDS.listSubmit}
          onClick={() => {
            void fetchMembers(currentCursor);
          }}
          type="button"
        >
          List members
        </button>
        <ul
          className="mt-3 flex flex-col gap-2"
          data-testid={ORGANIZATION_MEMBERS_WIRE_TEST_IDS.membersList}
        >
          {members.map((member) => (
            <li
              className="rounded border border-[--color-border] p-2"
              data-testid={memberRowTestId(member.accountId)}
              key={`wire-${member.accountId}`}
            >
              <div>{member.identifier}</div>
              <div data-testid={memberPermissionsTestId(member.accountId)}>
                {member.grantedPermissions
                  .map(
                    (grant: OrganizationMemberPermissionGrant) =>
                      `${grant.permission}:${grant.grant_source}`,
                  )
                  .join(",")}
              </div>
              <div
                className="text-xs text-[--color-fg-muted]"
                data-testid={memberGrantSourceTestId(member.accountId)}
              >
                sources:{" "}
                {[
                  ...new Set(
                    member.grantedPermissions.map(
                      (grant: OrganizationMemberPermissionGrant) =>
                        grant.grant_source,
                    ),
                  ),
                ].join(",")}
              </div>
              <div
                className="text-xs text-[--color-fg-muted]"
                data-testid={memberJoinedAtTestId(member.accountId)}
              >
                joined_at: {member.joinedAt}
              </div>
            </li>
          ))}
        </ul>
        {freshness ? (
          <p
            className="mt-3 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_MEMBERS_WIRE_TEST_IDS.listFreshness}
          >
            freshness: projection={freshness.projection} generated_at=
            {freshness.generated_at} cursor=
            {freshness.cursor ?? "<none>"} checkpoint=
            {freshness.checkpoint ?? "<none>"}
          </p>
        ) : null}
        {nextCursor !== null ? (
          <p
            className="mt-1 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_MEMBERS_WIRE_TEST_IDS.listNextCursor}
          >
            next_cursor: {nextCursor}
          </p>
        ) : null}
        {nextCursor !== null ? (
          <button
            className="mt-1 rounded border border-[--color-border] px-3 py-2 text-xs"
            data-testid={ORGANIZATION_MEMBERS_WIRE_TEST_IDS.listNextPage}
            onClick={() => {
              void listNextPage();
            }}
            type="button"
          >
            Next page
          </button>
        ) : null}
        {sourceLink ? (
          <p
            className="mt-1 text-xs text-[--color-fg-muted]"
            data-testid={ORGANIZATION_MEMBERS_WIRE_TEST_IDS.listSourceLink}
          >
            source: {sourceLink.event_family}/{sourceLink.event_kind}
          </p>
        ) : null}
      </section>
    </main>
  );
}
