"use client";

import { useEffect, useState } from "react";
import type { FormEvent, ReactNode } from "react";

import * as m from "@/i18n/paraglide/messages";
import type {
  PermissionCheckResponse,
  PrincipalRef,
  RoleScope,
} from "@/app/lib/generated/role-contract";
import {
  applyRole,
  checkPermission,
  createRole,
  deleteRole,
  editRole,
} from "@/app/lib/role-client";

interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

type ScopeKind = "account" | "organization" | "project";
type PrincipalKind = "account" | "role";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export default function Home(): ReactNode {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [roleMessage, setRoleMessage] = useState<string>("");
  const [roleResult, setRoleResult] = useState<string>("");

  useEffect(() => {
    let cancelled = false;
    fetch(`${API_URL}/health`, { credentials: "include" })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return (await response.json()) as HealthReport;
      })
      .then((data) => {
        if (!cancelled) {
          setReport(data);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const runRoleAction = async <T,>(
    label: string,
    action: () => Promise<T>,
  ): Promise<void> => {
    try {
      const response = await action();
      setRoleMessage(`${label}: ok`);
      setRoleResult(JSON.stringify(response, null, 2));
    } catch (reason: unknown) {
      const message = reason instanceof Error ? reason.message : String(reason);
      setRoleMessage(`${label}: ${message}`);
      setRoleResult("");
    }
  };

  const onCreateRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    void runRoleAction("create role", () =>
      createRole({
        scope: readScope(form, "scope_"),
        name: readRequired(form, "name"),
        permissions: readPermissions(form, "permissions"),
      }),
    );
  };

  const onEditRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    void runRoleAction("edit role", () =>
      editRole({
        role: {
          role_id: readRequired(form, "role_id"),
          scope: readScope(form, "scope_"),
        },
        name: readRequired(form, "name"),
        permissions: readPermissions(form, "permissions"),
      }),
    );
  };

  const onDeleteRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    void runRoleAction("delete role", () =>
      deleteRole({
        role: {
          role_id: readRequired(form, "role_id"),
          scope: readScope(form, "scope_"),
        },
      }),
    );
  };

  const onApplyRole = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    void runRoleAction("apply role", () =>
      applyRole({
        role: {
          role_id: readRequired(form, "role_id"),
          scope: readScope(form, "role_scope_"),
        },
        principal: readPrincipal(form, "principal_"),
        grant_scope: readScope(form, "grant_scope_"),
      }),
    );
  };

  const onCheckPermission = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    void runRoleAction<PermissionCheckResponse>("check permission", () =>
      checkPermission({
        principal: readPrincipal(form, "principal_"),
        permission: readRequired(form, "permission"),
        scope: readScope(form, "scope_"),
      }),
    );
  };

  return (
    <main className="flex min-h-screen flex-col gap-6 p-8">
      <h1 className="text-3xl font-semibold">{m.app_title()}</h1>
      <p className="text-[--color-fg-muted]">{m.app_placeholder()}</p>
      <section className="max-w-3xl rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4 font-mono">
        {report !== null ? (
          <pre className="m-0">{JSON.stringify(report, null, 2)}</pre>
        ) : error !== null ? (
          <span className="text-[--color-error]">
            {m.app_health_unreachable()}: {error}
          </span>
        ) : (
          <span className="text-[--color-fg-muted]">
            {m.app_health_loading()}
          </span>
        )}
      </section>

      <section className="grid gap-4 md:grid-cols-2">
        <RoleCard title="Create role" onSubmit={onCreateRole}>
          <ScopeFields prefix="scope_" />
          <LabeledInput
            name="name"
            label="Name"
            placeholder="workspace-admin"
          />
          <LabeledInput
            name="permissions"
            label="Permissions"
            placeholder="accounts.read,accounts.write"
          />
        </RoleCard>

        <RoleCard title="Edit role" onSubmit={onEditRole}>
          <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
          <ScopeFields prefix="scope_" />
          <LabeledInput
            name="name"
            label="Name"
            placeholder="workspace-admin"
          />
          <LabeledInput
            name="permissions"
            label="Permissions"
            placeholder="accounts.read,accounts.write"
          />
        </RoleCard>

        <RoleCard title="Delete role" onSubmit={onDeleteRole}>
          <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
          <ScopeFields prefix="scope_" />
        </RoleCard>

        <RoleCard title="Apply role" onSubmit={onApplyRole}>
          <LabeledInput name="role_id" label="Role id" placeholder="uuid" />
          <ScopeFields prefix="role_scope_" legend="Role scope" />
          <PrincipalFields prefix="principal_" />
          <ScopeFields prefix="grant_scope_" legend="Grant scope" />
        </RoleCard>

        <RoleCard title="Check permission" onSubmit={onCheckPermission}>
          <PrincipalFields prefix="principal_" />
          <LabeledInput
            name="permission"
            label="Permission"
            placeholder="accounts.read"
          />
          <ScopeFields prefix="scope_" />
        </RoleCard>
      </section>

      <section className="max-w-3xl rounded-md border border-[--color-border] bg-[--color-bg-surface] px-6 py-4">
        <h2 className="mb-2 text-lg font-medium">Role operation result</h2>
        <p className="mb-2 text-sm">{roleMessage}</p>
        <pre className="overflow-auto text-xs">{roleResult}</pre>
      </section>
    </main>
  );
}

function readRequired(form: FormData, name: string): string {
  const value = form.get(name);
  if (typeof value !== "string") {
    return "";
  }
  return value.trim();
}

function readPermissions(form: FormData, name: string): string[] {
  return readRequired(form, name)
    .split(",")
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
}

function readScope(form: FormData, prefix: string): RoleScope {
  const kind = readRequired(form, `${prefix}kind`) as ScopeKind;
  const id = readRequired(form, `${prefix}id`);
  if (kind === "organization") {
    return { scope: "organization", org_id: id };
  }
  if (kind === "project") {
    return { scope: "project", project_id: id };
  }
  return { scope: "account", account_id: id };
}

function readPrincipal(form: FormData, prefix: string): PrincipalRef {
  const kind = readRequired(form, `${prefix}kind`) as PrincipalKind;
  const id = readRequired(form, `${prefix}id`);
  if (kind === "role") {
    return { principal: "role", role_id: id };
  }
  return { principal: "account", account_id: id };
}

function RoleCard(props: {
  title: string;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  children: ReactNode;
}): ReactNode {
  return (
    <form
      className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4"
      onSubmit={props.onSubmit}
    >
      <h2 className="mb-2 text-lg font-medium">{props.title}</h2>
      <div className="space-y-2">{props.children}</div>
      <button
        className="mt-3 rounded border border-[--color-border] px-3 py-1 text-sm"
        type="submit"
      >
        Run
      </button>
    </form>
  );
}

function LabeledInput(props: {
  name: string;
  label: string;
  placeholder: string;
}): ReactNode {
  return (
    <label className="grid gap-1 text-sm">
      <span>{props.label}</span>
      <input
        className="rounded border border-[--color-border] bg-transparent px-2 py-1"
        name={props.name}
        placeholder={props.placeholder}
      />
    </label>
  );
}

function ScopeFields(props: { prefix: string; legend?: string }): ReactNode {
  return (
    <fieldset className="grid gap-2">
      <legend className="text-xs uppercase text-[--color-fg-muted]">
        {props.legend ?? "Scope"}
      </legend>
      <label className="grid gap-1 text-sm">
        <span>Scope kind</span>
        <select
          className="rounded border border-[--color-border] bg-transparent px-2 py-1"
          defaultValue="account"
          name={`${props.prefix}kind`}
        >
          <option value="account">account</option>
          <option value="organization">organization</option>
          <option value="project">project</option>
        </select>
      </label>
      <LabeledInput
        name={`${props.prefix}id`}
        label="Scope id"
        placeholder="uuid"
      />
    </fieldset>
  );
}

function PrincipalFields(props: { prefix: string }): ReactNode {
  return (
    <fieldset className="grid gap-2">
      <legend className="text-xs uppercase text-[--color-fg-muted]">
        Principal
      </legend>
      <label className="grid gap-1 text-sm">
        <span>Principal kind</span>
        <select
          className="rounded border border-[--color-border] bg-transparent px-2 py-1"
          defaultValue="account"
          name={`${props.prefix}kind`}
        >
          <option value="account">account</option>
          <option value="role">role</option>
        </select>
      </label>
      <LabeledInput
        name={`${props.prefix}id`}
        label="Principal id"
        placeholder="uuid"
      />
    </fieldset>
  );
}
