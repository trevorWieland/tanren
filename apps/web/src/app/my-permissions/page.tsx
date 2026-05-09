"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import {
  AccountRequestError,
  describeFailure,
  isInterfaceContractDriftFailure,
  myAccountCapabilities,
  myPermissions,
  permissionScopes,
  type AccountFailure,
  type MyPermissionEntry,
  type MyPermissionsResponse,
  type PermissionConstraintView,
  type PermissionGrantSource,
  type PermissionScopeView,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

function formatGrantSource(source: PermissionGrantSource): string {
  switch (source.kind) {
    case "direct":
      return m.myPermissions_sourceDirect();
    case "role_template":
      return `${m.myPermissions_sourceRoleTemplate()}: ${source.role_template}`;
  }
}

function formatConstraintSource(
  source: PermissionConstraintView["source"],
): string {
  switch (source) {
    case "organization_policy":
      return m.myPermissions_constraintSourceOrganizationPolicy();
    case "project_policy":
      return m.myPermissions_constraintSourceProjectPolicy();
  }
}

function stateBadgeClass(state: MyPermissionEntry["effective_state"]): string {
  if (state === "constrained") {
    return "border-[--color-error] text-[--color-error]";
  }
  return "border-[--color-success] text-[--color-success]";
}

function scopeDisplayLabel(scope: PermissionScopeView): string {
  switch (scope.kind) {
    case "organization":
      return m.myPermissions_organizationScopeLabel();
    case "project":
      return m.myPermissions_projectScopeLabel();
  }
}

function SourceReferenceValue({ reference }: { reference: string }): ReactNode {
  if (reference.startsWith("http://") || reference.startsWith("https://")) {
    return (
      <a
        href={reference}
        className="underline underline-offset-2"
        target="_blank"
        rel="noreferrer"
      >
        {reference}
      </a>
    );
  }
  return <code className="break-all text-xs sm:text-sm">{reference}</code>;
}

function isOrganizationScope(
  scope: PermissionScopeView,
): scope is Extract<PermissionScopeView, { kind: "organization" }> {
  return scope.kind === "organization";
}

function isProjectScope(
  scope: PermissionScopeView,
): scope is Extract<PermissionScopeView, { kind: "project" }> {
  return scope.kind === "project";
}

function ScopeCard({
  scopeLabel,
  scopeId,
  permissions,
}: {
  scopeLabel: string;
  scopeId: string;
  permissions: MyPermissionEntry[];
}): ReactNode {
  return (
    <article className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4">
      <h3 className="font-mono text-sm break-all">
        {scopeLabel}: {scopeId}
      </h3>
      <ul className="mt-3 space-y-3">
        {permissions.map((permission) => (
          <li
            key={`${scopeId}-${permission.permission}-${formatGrantSource(permission.grant_source)}`}
            className="rounded-md border border-[--color-border] bg-[--color-bg-elevated] p-3"
          >
            <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
              <code className="text-sm break-all">{permission.permission}</code>
              <span
                className={`inline-flex w-fit rounded-full border px-2 py-1 text-xs font-semibold uppercase ${stateBadgeClass(permission.effective_state)}`}
              >
                {permission.effective_state}
              </span>
            </div>
            <dl className="mt-3 grid gap-x-3 gap-y-1 text-sm sm:grid-cols-[auto_1fr]">
              <dt className="text-[--color-fg-muted]">
                {m.myPermissions_sourceLabel()}
              </dt>
              <dd className="break-all">
                {formatGrantSource(permission.grant_source)}
              </dd>
              <dt className="text-[--color-fg-muted]">
                {m.myPermissions_sourceReferenceLabel()}
              </dt>
              <dd>
                <SourceReferenceValue
                  reference={permission.grant_source_reference}
                />
              </dd>
              <dt className="text-[--color-fg-muted]">
                {m.myPermissions_constraintReasonLabel()}
              </dt>
              <dd>
                {permission.policy_constraint?.reason ??
                  m.myPermissions_constraintNone()}
              </dd>
              <dt className="text-[--color-fg-muted]">
                {m.myPermissions_constraintSourceLabel()}
              </dt>
              <dd>
                {permission.policy_constraint == null
                  ? m.myPermissions_constraintNone()
                  : formatConstraintSource(permission.policy_constraint.source)}
              </dd>
              <dt className="text-[--color-fg-muted]">
                {m.myPermissions_constraintSourceReferenceLabel()}
              </dt>
              <dd>
                {permission.policy_constraint == null ? (
                  m.myPermissions_constraintNone()
                ) : (
                  <SourceReferenceValue
                    reference={permission.policy_constraint.source_reference}
                  />
                )}
              </dd>
            </dl>
          </li>
        ))}
      </ul>
    </article>
  );
}

interface PermissionPageView {
  pageNumber: number;
  response: MyPermissionsResponse;
  organizationScopes: Extract<PermissionScopeView, { kind: "organization" }>[];
  projectScopes: Extract<PermissionScopeView, { kind: "project" }>[];
}

function permissionPageViews(
  pages: MyPermissionsResponse[],
): PermissionPageView[] {
  return pages.map((response, index) => {
    const scopes = permissionScopes(response);
    return {
      pageNumber: index + 1,
      response,
      organizationScopes: scopes.filter(isOrganizationScope),
      projectScopes: scopes.filter(isProjectScope),
    };
  });
}

function cursorOrNone(cursor: null | string | undefined): string {
  if (!cursor || cursor.trim() === "") {
    return m.myPermissions_metadataNone();
  }
  return cursor;
}

function unknownFailure(cause: unknown): AccountFailure {
  return {
    code: "internal_error",
    summary: cause instanceof Error ? cause.message : String(cause),
  };
}

export default function MyPermissionsPage(): ReactNode {
  const [pages, setPages] = useState<MyPermissionsResponse[]>([]);
  const [failure, setFailure] = useState<AccountFailure | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadMoreFailure, setLoadMoreFailure] = useState<AccountFailure | null>(
    null,
  );
  const pageViews = permissionPageViews(pages);
  const latestPage = pageViews.at(-1)?.response ?? null;
  const nextCursor = latestPage?.page.next_cursor ?? null;

  useEffect(() => {
    let cancelled = false;
    myAccountCapabilities()
      .then((response) => {
        if (cancelled) {
          return;
        }
        if (!response.can_view_my_permissions) {
          setFailure({
            code: "permission_denied",
            summary: "",
          });
          return;
        }
        return myPermissions().then((permissionsResponse) => {
          if (!cancelled) {
            setPages([permissionsResponse]);
          }
        });
      })
      .catch((cause: unknown) => {
        if (cancelled) {
          return;
        }
        if (cause instanceof AccountRequestError) {
          setFailure(cause.failure);
          return;
        }
        setFailure(unknownFailure(cause));
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  function loadNextPage(): void {
    if (!nextCursor || loadingMore) {
      return;
    }
    setLoadingMore(true);
    setLoadMoreFailure(null);
    myPermissions({ cursor: nextCursor })
      .then((response) => {
        setPages((current) => [...current, response]);
      })
      .catch((cause: unknown) => {
        if (cause instanceof AccountRequestError) {
          setLoadMoreFailure(cause.failure);
          return;
        }
        setLoadMoreFailure(unknownFailure(cause));
      })
      .finally(() => {
        setLoadingMore(false);
      });
  }

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-5xl flex-col gap-6 px-4 py-6 sm:px-6 sm:py-8">
      <header className="space-y-2">
        <Link
          href="/"
          className="inline-flex rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-1 text-sm hover:bg-[--color-bg-elevated]"
        >
          {m.myPermissions_backToHome()}
        </Link>
        <h1 className="text-2xl font-semibold sm:text-3xl">
          {m.myPermissions_title()}
        </h1>
        <p className="text-sm text-[--color-fg-muted] sm:text-base">
          {m.myPermissions_subtitle()}
        </p>
      </header>

      {loading ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
          {m.myPermissions_loading()}
        </section>
      ) : failure !== null ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm">
          <p className="m-0 text-[--color-error]">{describeFailure(failure)}</p>
          {isInterfaceContractDriftFailure(failure) ? (
            <dl className="mt-2 grid gap-x-3 gap-y-1 font-mono text-[--color-fg-muted] sm:grid-cols-[auto_1fr]">
              <dt>type:</dt>
              <dd>interface_contract_drift</dd>
              <dt>raw_code:</dt>
              <dd className="break-all">{failure.code}</dd>
              <dt>status:</dt>
              <dd>{failure.status}</dd>
            </dl>
          ) : (
            <p className="m-0 mt-1 font-mono text-[--color-fg-muted]">
              code: {failure.code}
            </p>
          )}
        </section>
      ) : pageViews.length === 0 ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
          {m.myPermissions_empty()}
        </section>
      ) : (
        <div className="space-y-6">
          <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm">
            <dl className="grid gap-x-3 gap-y-1 sm:grid-cols-[auto_1fr]">
              <dt className="text-[--color-fg-muted]">
                {m.myPermissions_metadataPagesLoadedLabel()}
              </dt>
              <dd>{pageViews.length}</dd>
              <dt className="text-[--color-fg-muted]">
                {m.myPermissions_metadataNextCursorLabel()}
              </dt>
              <dd className="break-all">{cursorOrNone(nextCursor)}</dd>
            </dl>
            {nextCursor ? (
              <div className="mt-4 space-y-2">
                <button
                  type="button"
                  onClick={loadNextPage}
                  disabled={loadingMore}
                  className="inline-flex rounded-md border border-[--color-border] bg-[--color-bg-elevated] px-3 py-2 text-sm hover:bg-[--color-bg-surface] disabled:cursor-not-allowed disabled:opacity-70"
                >
                  {loadingMore
                    ? m.myPermissions_loadingMore()
                    : m.myPermissions_loadMore()}
                </button>
                {loadMoreFailure ? (
                  <div className="space-y-1 text-sm">
                    <p className="m-0 text-[--color-error]">
                      {describeFailure(loadMoreFailure)}
                    </p>
                    {isInterfaceContractDriftFailure(loadMoreFailure) ? (
                      <p className="m-0 font-mono text-[--color-fg-muted]">
                        contract drift: {loadMoreFailure.code} (HTTP{" "}
                        {loadMoreFailure.status})
                      </p>
                    ) : null}
                  </div>
                ) : null}
              </div>
            ) : (
              <p className="mt-4 m-0 text-sm text-[--color-fg-muted]">
                {m.myPermissions_noMorePages()}
              </p>
            )}
          </section>

          <div className="space-y-6">
            {pageViews.map((pageView) => (
              <section
                key={`permissions-page-${pageView.pageNumber}`}
                data-testid={`permissions-page-${pageView.pageNumber}`}
                className="space-y-4 rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4"
              >
                <h2 className="text-base font-semibold">
                  {m.myPermissions_metadataTitle()} #{pageView.pageNumber}
                </h2>
                <dl className="grid gap-x-3 gap-y-1 text-sm sm:grid-cols-[auto_1fr]">
                  <dt className="text-[--color-fg-muted]">
                    {m.myPermissions_metadataProjectionLabel()}
                  </dt>
                  <dd className="break-all">
                    {pageView.response.freshness.projection}
                  </dd>
                  <dt className="text-[--color-fg-muted]">
                    {m.myPermissions_metadataGeneratedAtLabel()}
                  </dt>
                  <dd>
                    {new Date(
                      pageView.response.freshness.generated_at,
                    ).toLocaleString()}
                  </dd>
                  <dt className="text-[--color-fg-muted]">
                    {m.myPermissions_metadataCheckpointLabel()}
                  </dt>
                  <dd className="break-all">
                    {cursorOrNone(pageView.response.freshness.checkpoint)}
                  </dd>
                  <dt className="text-[--color-fg-muted]">
                    {m.myPermissions_metadataStalenessLabel()}
                  </dt>
                  <dd>{pageView.response.freshness.staleness}</dd>
                  <dt className="text-[--color-fg-muted]">
                    {m.myPermissions_metadataLimitLabel()}
                  </dt>
                  <dd>{pageView.response.page.limit}</dd>
                  <dt className="text-[--color-fg-muted]">
                    {m.myPermissions_metadataReturnedLabel()}
                  </dt>
                  <dd>{pageView.response.page.returned}</dd>
                  <dt className="text-[--color-fg-muted]">
                    {m.myPermissions_metadataRequestCursorLabel()}
                  </dt>
                  <dd className="break-all">
                    {cursorOrNone(pageView.response.page.request_cursor)}
                  </dd>
                  <dt className="text-[--color-fg-muted]">
                    {m.myPermissions_metadataNextCursorLabel()}
                  </dt>
                  <dd className="break-all">
                    {cursorOrNone(pageView.response.page.next_cursor)}
                  </dd>
                </dl>

                <div className="grid gap-6 lg:grid-cols-2">
                  <section className="space-y-3">
                    <h3 className="text-lg font-semibold">
                      {m.myPermissions_organizationsTitle()}
                    </h3>
                    {pageView.organizationScopes.length === 0 ? (
                      <p className="rounded-md border border-[--color-border] bg-[--color-bg-elevated] p-4 text-sm text-[--color-fg-muted]">
                        {m.myPermissions_organizationsEmpty()}
                      </p>
                    ) : (
                      <div className="space-y-3">
                        {pageView.organizationScopes.map((scope) => (
                          <ScopeCard
                            key={`${scope.kind}-${scope.scope_id}`}
                            scopeLabel={scopeDisplayLabel(scope)}
                            scopeId={scope.scope_id}
                            permissions={scope.permissions}
                          />
                        ))}
                      </div>
                    )}
                  </section>

                  <section className="space-y-3">
                    <h3 className="text-lg font-semibold">
                      {m.myPermissions_projectsTitle()}
                    </h3>
                    {pageView.projectScopes.length === 0 ? (
                      <p className="rounded-md border border-[--color-border] bg-[--color-bg-elevated] p-4 text-sm text-[--color-fg-muted]">
                        {m.myPermissions_projectsEmpty()}
                      </p>
                    ) : (
                      <div className="space-y-3">
                        {pageView.projectScopes.map((scope) => (
                          <ScopeCard
                            key={`${scope.kind}-${scope.scope_id}`}
                            scopeLabel={scopeDisplayLabel(scope)}
                            scopeId={scope.scope_id}
                            permissions={scope.permissions}
                          />
                        ))}
                      </div>
                    )}
                  </section>
                </div>
              </section>
            ))}
          </div>
        </div>
      )}
    </main>
  );
}
