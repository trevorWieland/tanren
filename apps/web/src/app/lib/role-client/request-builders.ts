import type {
  AccountPrincipalRef,
  ApplyRoleRequest,
  CreateRoleRequest,
  DeleteRoleRequest,
  EditRoleRequest,
  PermissionCheckRequest,
  PermissionScope,
  RoleScope,
} from "../generated/role-contract";
import type { PermissionCheckRolePrincipalRejectionRequest } from "./operations";
import {
  permissionScopeFromRoleScope,
  readAccountPrincipalRef,
  readPermissionBundle,
  readPermissionNameField,
  readPermissionScope,
  readPrincipalKindField,
  readRoleIdField,
  readRoleNameField,
  readRolePrincipalRejectionRef,
  readRoleScope,
  roleScopeFromPermissionScope,
} from "./form-parsing";

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
  request: ApplyRoleRequest;
  context: RoleRequestContextInput;
}

export type PermissionCheckFormSubmission =
  | {
      principalKind: "account";
      request: PermissionCheckRequest;
      context: RoleRequestContextInput;
    }
  | {
      principalKind: "role";
      request: PermissionCheckRolePrincipalRejectionRequest;
      context: RoleRequestContextInput;
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
  const principalKind = readPrincipalKindField(form, "principal_");
  switch (principalKind) {
    case "role":
      return {
        principalKind: "role",
        request: {
          principal: readRolePrincipalRejectionRef(form, "principal_"),
          permission,
          scope,
        },
        context: {
          roleScope: roleScopeFromPermissionScope(scope),
          grantScope: scope,
        },
      };
    case "account": {
      const principal = readAccountPrincipalRef(form, "principal_");
      return {
        principalKind: "account",
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
    default:
      return assertNever(principalKind, "permission check principal kind");
  }
}

function assertNever(value: never, context: string): never {
  throw new Error(`${context} received unsupported variant: ${String(value)}`);
}

export { permissionScopeFromRoleScope, roleScopeFromPermissionScope };
