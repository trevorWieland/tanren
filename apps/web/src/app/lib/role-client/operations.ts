import type {
  ApplyRoleRequest,
  ApplyRoleResponse,
  CreateRoleRequest,
  CreateRoleResponse,
  DeleteRoleRequest,
  DeleteRoleResponse,
  EditRoleRequest,
  EditRoleResponse,
  PermissionCheckRequest,
  PermissionCheckResponse,
  PermissionName,
  PermissionScope,
  RoleAdminAction,
  RoleReadModelRequest,
  RoleReadModelResponse,
  RolePrincipalRejectionRef,
} from "../generated/role-contract";
import {
  parseApplyRoleResponse,
  parseCreateRoleResponse,
  parseDeleteRoleResponse,
  parseEditRoleResponse,
  parsePermissionCheckResponse,
  parseRoleReadModelResponse,
} from "../generated/role-contract";

import {
  type RoleCapabilitySnapshot,
  requireRoleActionSnapshot,
} from "./capabilities";
import { toRoleTransportError } from "./errors";
import { postRoleJson } from "./transport";

export interface PermissionCheckRolePrincipalRejectionRequest {
  principal: RolePrincipalRejectionRef;
  permission: PermissionName;
  scope: PermissionScope;
}

interface RolePostEndpointMap {
  "/roles": {
    request: CreateRoleRequest;
    response: CreateRoleResponse;
    action: "create_role";
  };
  "/roles/edit": {
    request: EditRoleRequest;
    response: EditRoleResponse;
    action: "edit_role";
  };
  "/roles/delete": {
    request: DeleteRoleRequest;
    response: DeleteRoleResponse;
    action: "delete_role";
  };
  "/roles/apply": {
    request: ApplyRoleRequest;
    response: ApplyRoleResponse;
    action: "apply_role";
  };
  "/permissions/check": {
    request:
      | PermissionCheckRequest
      | PermissionCheckRolePrincipalRejectionRequest;
    response: PermissionCheckResponse;
    action: "check_permission";
  };
  "/roles/read-model": {
    request: RoleReadModelRequest;
    response: RoleReadModelResponse;
    action: "read_roles";
  };
}

type RolePostPath = keyof RolePostEndpointMap;
type ResponseDecoder<TResponse> = (payload: unknown) => TResponse;

const ROLE_POST_DECODERS: {
  [K in RolePostPath]: ResponseDecoder<RolePostEndpointMap[K]["response"]>;
} = {
  "/roles": parseCreateRoleResponse,
  "/roles/edit": parseEditRoleResponse,
  "/roles/delete": parseDeleteRoleResponse,
  "/roles/apply": parseApplyRoleResponse,
  "/permissions/check": parsePermissionCheckResponse,
  "/roles/read-model": parseRoleReadModelResponse,
};

const ROLE_POST_ACTIONS: {
  [K in RolePostPath]: RolePostEndpointMap[K]["action"];
} = {
  "/roles": "create_role",
  "/roles/edit": "edit_role",
  "/roles/delete": "delete_role",
  "/roles/apply": "apply_role",
  "/permissions/check": "check_permission",
  "/roles/read-model": "read_roles",
};

async function postRoleAuthorizedEndpoint<K extends RolePostPath>(
  path: K,
  body: RolePostEndpointMap[K]["request"],
  snapshot: RoleCapabilitySnapshot,
  options?: { signal?: AbortSignal },
): Promise<RolePostEndpointMap[K]["response"]> {
  const action: RoleAdminAction = ROLE_POST_ACTIONS[path];
  const authorized = requireRoleActionSnapshot(snapshot, action);
  const payload = await postRoleJson(path, body, {
    csrfToken: authorized.csrfToken,
    ...(options?.signal === undefined ? {} : { signal: options.signal }),
  });

  const decode = ROLE_POST_DECODERS[path];
  try {
    return decode(payload);
  } catch (cause: unknown) {
    throw toRoleTransportError(cause, `unexpected ${path} response payload`);
  }
}

export function createRole(
  request: CreateRoleRequest,
  snapshot: RoleCapabilitySnapshot,
): Promise<CreateRoleResponse> {
  return postRoleAuthorizedEndpoint("/roles", request, snapshot);
}

export function editRole(
  request: EditRoleRequest,
  snapshot: RoleCapabilitySnapshot,
): Promise<EditRoleResponse> {
  return postRoleAuthorizedEndpoint("/roles/edit", request, snapshot);
}

export function deleteRole(
  request: DeleteRoleRequest,
  snapshot: RoleCapabilitySnapshot,
): Promise<DeleteRoleResponse> {
  return postRoleAuthorizedEndpoint("/roles/delete", request, snapshot);
}

export function applyRole(
  request: ApplyRoleRequest,
  snapshot: RoleCapabilitySnapshot,
): Promise<ApplyRoleResponse> {
  return postRoleAuthorizedEndpoint("/roles/apply", request, snapshot);
}

export function checkPermission(
  request: PermissionCheckRequest,
  snapshot: RoleCapabilitySnapshot,
): Promise<PermissionCheckResponse> {
  return postRoleAuthorizedEndpoint("/permissions/check", request, snapshot);
}

export function checkPermissionRolePrincipalRejection(
  request: PermissionCheckRolePrincipalRejectionRequest,
  snapshot: RoleCapabilitySnapshot,
): Promise<PermissionCheckResponse> {
  return postRoleAuthorizedEndpoint("/permissions/check", request, snapshot);
}

export function readRoleModel(
  request: RoleReadModelRequest,
  snapshot: RoleCapabilitySnapshot,
  signal?: AbortSignal,
): Promise<RoleReadModelResponse> {
  return postRoleAuthorizedEndpoint(
    "/roles/read-model",
    request,
    snapshot,
    signal === undefined ? undefined : { signal },
  );
}
