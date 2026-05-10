import type {
  AccountPrincipalRef,
  ApplyRoleRequest,
  CreateRoleRequest,
  DeleteRoleRequest,
  EditRoleRequest,
  PermissionCheckRequest,
  PermissionScope,
  RoleReadModelRequest,
  RoleScope,
} from "../generated/role-contract";
import type { PermissionCheckRolePrincipalRejectionRequest } from "./operations";
import { ROLE_READ_MODEL_PAGE_MAX as ROLE_READ_MODEL_PAGE_MAX_VALUE } from "../generated/role-contract";
import {
  permissionScopeFromRoleScope,
  readAccountPrincipalRef,
  readPermissionBundle,
  readPermissionNameField,
  readPermissionScope,
  readRoleIdField,
  readRoleNameField,
  readRolePrincipalRejectionRef,
  readRoleScope,
  roleScopeFromPermissionScope,
} from "./form-parsing";
import { toRoleValidationError } from "./errors";

export interface RoleRequestContextInput {
  roleScope: RoleScope;
  grantScope: PermissionScope;
  grantPrincipal?: AccountPrincipalRef;
}

export interface CreateRoleFormSubmission {
  request: CreateRoleRequest;
  context: RoleRequestContextInput;
}

export interface EditRoleFormSubmission {
  request: EditRoleRequest;
  context: RoleRequestContextInput;
}

export interface DeleteRoleFormSubmission {
  request: DeleteRoleRequest;
  context: RoleRequestContextInput;
}

export interface ApplyRoleFormSubmission {
  request: ApplyRoleAccountPrincipalRequest;
  context: RoleRequestContextInput;
}

export interface PermissionCheckFormSubmission {
  request: PermissionCheckAccountPrincipalRequest;
  context: RoleRequestContextInput;
}

export interface PermissionCheckRolePrincipalRejectionFormSubmission {
  request: PermissionCheckRolePrincipalRejectionRequest;
  context: Omit<RoleRequestContextInput, "grantPrincipal">;
}

export interface RoleReadModelRequestInput {
  roleScope: RoleScope;
  roleCursor: RoleReadModelRequest["role_cursor"];
  roleLimit: number | null;
  grantPrincipal: AccountPrincipalRef;
  grantScope: PermissionScope;
  grantCursor: RoleReadModelRequest["grant_cursor"];
  grantLimit: number | null;
}

export type ApplyRoleAccountPrincipalRequest = Omit<
  ApplyRoleRequest,
  "principal"
> & {
  principal: AccountPrincipalRef;
};

export type PermissionCheckAccountPrincipalRequest = Omit<
  PermissionCheckRequest,
  "principal"
> & {
  principal: AccountPrincipalRef;
};

export type RoleReadModelAccountPrincipalRequest = Omit<
  RoleReadModelRequest,
  "grant_principal"
> & {
  grant_principal: AccountPrincipalRef;
};

export function buildCreateRoleRequest(
  form: FormData,
): CreateRoleFormSubmission {
  const roleScope = readRoleScope(form, "scope_");
  return {
    request: {
      scope: roleScope,
      name: readRoleNameField(form, "name"),
      permissions: readPermissionBundle(form, "permissions"),
    },
    context: {
      roleScope,
      grantScope: permissionScopeFromRoleScope(roleScope),
    },
  };
}

export function buildEditRoleRequest(form: FormData): EditRoleFormSubmission {
  const roleScope = readRoleScope(form, "scope_");
  return {
    request: {
      role: {
        role_id: readRoleIdField(form, "role_id"),
        scope: roleScope,
      },
      name: readRoleNameField(form, "name"),
      permissions: readPermissionBundle(form, "permissions"),
    },
    context: {
      roleScope,
      grantScope: permissionScopeFromRoleScope(roleScope),
    },
  };
}

export function buildDeleteRoleRequest(
  form: FormData,
): DeleteRoleFormSubmission {
  const roleScope = readRoleScope(form, "scope_");
  return {
    request: {
      role: {
        role_id: readRoleIdField(form, "role_id"),
        scope: roleScope,
      },
    },
    context: {
      roleScope,
      grantScope: permissionScopeFromRoleScope(roleScope),
    },
  };
}

export function buildApplyRoleRequest(form: FormData): ApplyRoleFormSubmission {
  const roleScope = readRoleScope(form, "role_scope_");
  const principal = readAccountPrincipalRef(form, "principal_");
  const grantScope = readPermissionScope(form, "grant_scope_");
  return {
    request: {
      role: {
        role_id: readRoleIdField(form, "role_id"),
        scope: roleScope,
      },
      principal,
      grant_scope: grantScope,
    },
    context: {
      roleScope,
      grantPrincipal: principal,
      grantScope,
    },
  };
}

export function buildPermissionCheckRequest(
  form: FormData,
): PermissionCheckFormSubmission {
  const scope = readPermissionScope(form, "scope_");
  const permission = readPermissionNameField(form, "permission");
  const principal = readAccountPrincipalRef(form, "principal_");
  return {
    request: {
      principal,
      permission,
      scope,
    },
    context: {
      roleScope: roleScopeFromPermissionScope(scope),
      grantPrincipal: principal,
      grantScope: scope,
    },
  };
}

export function buildPermissionCheckRolePrincipalRejectionRequest(
  form: FormData,
): PermissionCheckRolePrincipalRejectionFormSubmission {
  const scope = readPermissionScope(form, "scope_");
  return {
    request: {
      principal: readRolePrincipalRejectionRef(form, "principal_"),
      permission: readPermissionNameField(form, "permission"),
      scope,
    },
    context: {
      roleScope: roleScopeFromPermissionScope(scope),
      grantScope: scope,
    },
  };
}

export function buildRoleReadModelRequest(
  input: RoleReadModelRequestInput,
): RoleReadModelAccountPrincipalRequest {
  return {
    role_scope: input.roleScope,
    role_cursor: input.roleCursor,
    role_limit: validateRoleReadModelLimit(input.roleLimit, "role_limit"),
    grant_principal: input.grantPrincipal,
    grant_scope: input.grantScope,
    grant_cursor: input.grantCursor,
    grant_limit: validateRoleReadModelLimit(input.grantLimit, "grant_limit"),
  };
}

function validateRoleReadModelLimit(
  value: number | null,
  field: "role_limit" | "grant_limit",
): number | null {
  if (value === null) {
    return null;
  }
  if (!Number.isInteger(value) || value <= 0) {
    throw toRoleValidationError(`${field} must be a positive integer`);
  }
  if (value > ROLE_READ_MODEL_PAGE_MAX_VALUE) {
    throw toRoleValidationError(
      `${field} must be less than or equal to ${String(ROLE_READ_MODEL_PAGE_MAX_VALUE)}`,
    );
  }
  return value;
}

export { permissionScopeFromRoleScope, roleScopeFromPermissionScope };
