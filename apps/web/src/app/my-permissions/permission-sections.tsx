import type { ReactNode } from "react";

import {
  describeFailure,
  type AccountFailure,
  type MyPermissionEntry,
} from "@/app/lib/account-client";
import * as m from "@/i18n/paraglide/messages";

import {
  cursorOrNone,
  formatConstraintSource,
  formatGrantSource,
  scopeDisplayLabel,
  stateBadgeClass,
} from "./permission-formatting";
import type { PermissionPageView } from "./use-my-permissions-pages";

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

function ScopeSection({
  title,
  emptyMessage,
  scopes,
}: {
  title: string;
  emptyMessage: string;
  scopes:
    | PermissionPageView["organizationScopes"]
    | PermissionPageView["projectScopes"];
}): ReactNode {
  return (
    <section className="space-y-3">
      <h3 className="text-lg font-semibold">{title}</h3>
      {scopes.length === 0 ? (
        <p className="rounded-md border border-[--color-border] bg-[--color-bg-elevated] p-4 text-sm text-[--color-fg-muted]">
          {emptyMessage}
        </p>
      ) : (
        <div className="space-y-3">
          {scopes.map((scope) => (
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
  );
}

export function PermissionMetadataSection({
  pagesLoaded,
  nextCursor,
  loadingMore,
  loadMoreFailure,
  onLoadMore,
}: {
  pagesLoaded: number;
  nextCursor: null | string;
  loadingMore: boolean;
  loadMoreFailure: AccountFailure | null;
  onLoadMore: () => void;
}): ReactNode {
  return (
    <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm">
      <dl className="grid gap-x-3 gap-y-1 sm:grid-cols-[auto_1fr]">
        <dt className="text-[--color-fg-muted]">
          {m.myPermissions_metadataPagesLoadedLabel()}
        </dt>
        <dd>{pagesLoaded}</dd>
        <dt className="text-[--color-fg-muted]">
          {m.myPermissions_metadataNextCursorLabel()}
        </dt>
        <dd className="break-all">{cursorOrNone(nextCursor)}</dd>
      </dl>
      {nextCursor ? (
        <div className="mt-4 space-y-2">
          <button
            type="button"
            onClick={onLoadMore}
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
              <p className="m-0 font-mono text-[--color-fg-muted]">
                code: {loadMoreFailure.code}
              </p>
            </div>
          ) : null}
        </div>
      ) : (
        <p className="mt-4 m-0 text-sm text-[--color-fg-muted]">
          {m.myPermissions_noMorePages()}
        </p>
      )}
    </section>
  );
}

export function PermissionFailureSection({
  failure,
}: {
  failure: AccountFailure;
}): ReactNode {
  return (
    <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm">
      <p className="m-0 text-[--color-error]">{describeFailure(failure)}</p>
      <p className="m-0 mt-1 font-mono text-[--color-fg-muted]">
        code: {failure.code}
      </p>
    </section>
  );
}

export function PermissionPageSection({
  pageView,
}: {
  pageView: PermissionPageView;
}): ReactNode {
  return (
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
        <dd className="break-all">{pageView.response.read_metadata.source}</dd>
        <dt className="text-[--color-fg-muted]">
          {m.myPermissions_metadataGeneratedAtLabel()}
        </dt>
        <dd>
          {new Date(
            pageView.response.read_metadata.generated_at,
          ).toLocaleString()}
        </dd>
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
        <ScopeSection
          title={m.myPermissions_organizationsTitle()}
          emptyMessage={m.myPermissions_organizationsEmpty()}
          scopes={pageView.organizationScopes}
        />
        <ScopeSection
          title={m.myPermissions_projectsTitle()}
          emptyMessage={m.myPermissions_projectsEmpty()}
          scopes={pageView.projectScopes}
        />
      </div>
    </section>
  );
}
