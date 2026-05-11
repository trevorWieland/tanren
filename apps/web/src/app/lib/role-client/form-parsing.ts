import type {
  AccountId,
  AccountPrincipalRef,
  OrgId,
  PermissionName,
  PermissionScope,
  PrincipalRef,
  ProjectId,
  RoleId,
  RolePrincipalRejectionRef,
  RoleScope,
} from "../generated/role-contract";
import {
  asAccountId,
  asOrgId,
  asPermissionName,
  asProjectId,
  asRoleId,
  parsePrincipalKind,
  parseRoleScopeKind,
} from "../generated/role-contract";

import { toRoleValidationError } from "./errors";

export type ScopeKind = RoleScope["scope"];
export type PrincipalKind =
  | AccountPrincipalRef["principal"]
  | RolePrincipalRejectionRef["principal"];

export function readRequiredField(form: FormData, name: string): string {
  const value = form.get(name);
  if (typeof value !== "string") {
    return "";
  }
  return value.trim();
}

function readNonEmptyField(form: FormData, name: string): string {
  const value = readRequiredField(form, name);
  if (value.length === 0) {
    throw toRoleValidationError(`missing required field: ${name}`);
  }
  return value;
}

export function readPermissionBundle(
  form: FormData,
  name: string,
): PermissionName[] {
  const parts = readNonEmptyField(form, name)
    .split(",")
    .map((part) => part.trim());
  if (parts.some((part) => part.length === 0)) {
    throw toRoleValidationError(
      `field ${name} must contain non-empty permission names`,
    );
  }
  const permissions: PermissionName[] = [];
  const seen = new Set<string>();
  for (const part of parts) {
    if (seen.has(part)) {
      continue;
    }
    seen.add(part);
    permissions.push(asPermissionName(part));
  }
  return permissions;
}

export function readRoleScope(form: FormData, prefix: string): RoleScope {
  const kind = parseScopeKind(readNonEmptyField(form, `${prefix}kind`));
  const id = readNonEmptyField(form, `${prefix}id`);
  switch (kind) {
    case "organization":
      return { scope: "organization", org_id: asOrgIdValue(id) };
    case "project":
      return { scope: "project", project_id: asProjectIdValue(id) };
    case "account":
      return { scope: "account", account_id: asAccountIdValue(id) };
    default:
      return assertNever(kind, "scope kind");
  }
}

export function readPermissionScope(
  form: FormData,
  prefix: string,
): PermissionScope {
  const scope = readRoleScope(form, prefix);
  return permissionScopeFromRoleScope(scope);
}

export function readPrincipalKindField(
  form: FormData,
  prefix: string,
): PrincipalKind {
  return parseFormPrincipalKind(readNonEmptyField(form, `${prefix}kind`));
}

export function readAccountPrincipalRef(
  form: FormData,
  prefix: string,
): AccountPrincipalRef {
  const kind = readPrincipalKindField(form, prefix);
  switch (kind) {
    case "account":
      return {
        principal: "account",
        account_id: asAccountIdValue(readNonEmptyField(form, `${prefix}id`)),
      };
    case "role":
      throw toRoleValidationError(
        `field ${prefix}kind must be account for this request`,
      );
    default:
      return assertNever(kind, "account principal kind");
  }
}

export function readRolePrincipalRejectionRef(
  form: FormData,
  prefix: string,
): RolePrincipalRejectionRef {
  const kind = readPrincipalKindField(form, prefix);
  switch (kind) {
    case "role":
      return {
        principal: "role",
        role_id: asRoleIdValue(readNonEmptyField(form, `${prefix}id`)),
      };
    case "account":
      throw toRoleValidationError(
        `field ${prefix}kind must be role for rejection witness requests`,
      );
    default:
      return assertNever(kind, "role principal kind");
  }
}

export function readRoleIdField(form: FormData, name: string): RoleId {
  return asRoleIdValue(readNonEmptyField(form, name));
}

export function readPermissionNameField(
  form: FormData,
  name: string,
): PermissionName {
  return asPermissionName(readNonEmptyField(form, name));
}

export function readRoleNameField(form: FormData, name: string): string {
  return readNonEmptyField(form, name);
}

export function permissionScopeFromRoleScope(
  scope: RoleScope,
): PermissionScope {
  switch (scope.scope) {
    case "organization":
      return { scope: "organization", org_id: scope.org_id };
    case "project":
      return { scope: "project", project_id: scope.project_id };
    case "account":
      return { scope: "account", account_id: scope.account_id };
    default:
      return assertNever(scope, "role scope");
  }
}

export function roleScopeFromPermissionScope(
  scope: PermissionScope,
): RoleScope {
  switch (scope.scope) {
    case "organization":
      return { scope: "organization", org_id: scope.org_id };
    case "project":
      return { scope: "project", project_id: scope.project_id };
    case "account":
      return { scope: "account", account_id: scope.account_id };
    default:
      return assertNever(scope, "permission scope");
  }
}

function parseScopeKind(value: string): ScopeKind {
  try {
    return parseRoleScopeKind(value);
  } catch {
    throw toRoleValidationError(
      `scope kind must be account, organization, or project: ${value}`,
    );
  }
}

function parseFormPrincipalKind(value: string): PrincipalRef["principal"] {
  try {
    return parsePrincipalKind(value);
  } catch {
    throw toRoleValidationError(
      `principal kind must be account or role: ${value}`,
    );
  }
}

function asRoleIdValue(value: string): RoleId {
  return asRoleId(value);
}

function asAccountIdValue(value: string): AccountId {
  return asAccountId(value);
}

function asOrgIdValue(value: string): OrgId {
  return asOrgId(value);
}

function asProjectIdValue(value: string): ProjectId {
  return asProjectId(value);
}

function assertNever(value: never, context: string): never {
  throw new Error(`${context} received unsupported variant: ${String(value)}`);
}
