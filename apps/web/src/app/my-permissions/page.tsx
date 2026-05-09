"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import {
  AccountRequestError,
  describeFailure,
  myPermissions,
  permissionScopes,
  type InterfaceError,
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
            </dl>
          </li>
        ))}
      </ul>
    </article>
  );
}

export default function MyPermissionsPage(): ReactNode {
  const [data, setData] = useState<MyPermissionsResponse | null>(null);
  const [failure, setFailure] = useState<InterfaceError | null>(null);
  const [loading, setLoading] = useState(true);
  const scopes = data === null ? [] : permissionScopes(data);
  const organizationScopes = scopes.filter(isOrganizationScope);
  const projectScopes = scopes.filter(isProjectScope);

  useEffect(() => {
    let cancelled = false;
    myPermissions()
      .then((response) => {
        if (!cancelled) {
          setData(response);
        }
      })
      .catch((cause: unknown) => {
        if (cancelled) {
          return;
        }
        if (cause instanceof AccountRequestError) {
          setFailure(cause.failure);
          return;
        }
        setFailure({
          code: "internal_error",
          summary: cause instanceof Error ? cause.message : String(cause),
        });
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
          <p className="m-0 mt-1 font-mono text-[--color-fg-muted]">
            code: {failure.code}
          </p>
        </section>
      ) : data === null ? (
        <section className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
          {m.myPermissions_empty()}
        </section>
      ) : (
        <div className="grid gap-6 lg:grid-cols-2">
          <section className="space-y-3">
            <h2 className="text-lg font-semibold">
              {m.myPermissions_organizationsTitle()}
            </h2>
            {organizationScopes.length === 0 ? (
              <p className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
                {m.myPermissions_organizationsEmpty()}
              </p>
            ) : (
              <div className="space-y-3">
                {organizationScopes.map((scope) => (
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
            <h2 className="text-lg font-semibold">
              {m.myPermissions_projectsTitle()}
            </h2>
            {projectScopes.length === 0 ? (
              <p className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4 text-sm text-[--color-fg-muted]">
                {m.myPermissions_projectsEmpty()}
              </p>
            ) : (
              <div className="space-y-3">
                {projectScopes.map((scope) => (
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
      )}
    </main>
  );
}
