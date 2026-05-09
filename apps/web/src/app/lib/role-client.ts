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
  RoleAdminCapabilities,
  RoleFailureBody,
} from "./generated/role-contract";
import { parseRoleFailure } from "./generated/role-contract";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export class RoleRequestError extends Error {
  readonly failure: RoleFailureBody;

  constructor(failure: RoleFailureBody) {
    super(`${failure.code}: ${failure.summary}`);
    this.failure = failure;
    this.name = "RoleRequestError";
  }
}

type RoleCommandRequest =
  | CreateRoleRequest
  | EditRoleRequest
  | DeleteRoleRequest
  | ApplyRoleRequest
  | PermissionCheckRequest;

async function postRoleJson<TResponse>(
  path: string,
  body: RoleCommandRequest,
  csrfToken?: string,
): Promise<TResponse> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        ...(csrfToken !== undefined ? { "x-csrf-token": csrfToken } : {}),
      },
      body: JSON.stringify(body),
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new RoleRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  if (!response.ok) {
    let payload: unknown = undefined;
    try {
      payload = (await response.json()) as unknown;
    } catch {
      payload = undefined;
    }
    throw new RoleRequestError(parseRoleFailure(payload));
  }

  return (await response.json()) as TResponse;
}

export interface RoleCapabilitySnapshot {
  capabilities: RoleAdminCapabilities;
  csrfToken: string;
}

export async function fetchRoleCapabilities(): Promise<RoleCapabilitySnapshot> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}/roles/capabilities`, {
      method: "GET",
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new RoleRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  let payload: unknown = undefined;
  try {
    payload = (await response.json()) as unknown;
  } catch {
    payload = undefined;
  }

  if (!response.ok) {
    throw new RoleRequestError(parseRoleFailure(payload));
  }

  const typed = payload as {
    capabilities?: RoleAdminCapabilities;
    csrf_token?: string;
  };
  if (typed.capabilities === undefined) {
    throw new RoleRequestError({
      code: "transport_error",
      summary: "unexpected capability payload",
    });
  }
  if (typeof typed.csrf_token !== "string" || typed.csrf_token.length === 0) {
    throw new RoleRequestError({
      code: "transport_error",
      summary: "missing csrf token in capability payload",
    });
  }
  return {
    capabilities: typed.capabilities,
    csrfToken: typed.csrf_token,
  };
}

export function createRole(
  request: CreateRoleRequest,
  csrfToken: string,
): Promise<CreateRoleResponse> {
  return postRoleJson<CreateRoleResponse>("/roles", request, csrfToken);
}

export function editRole(
  request: EditRoleRequest,
  csrfToken: string,
): Promise<EditRoleResponse> {
  return postRoleJson<EditRoleResponse>("/roles/edit", request, csrfToken);
}

export function deleteRole(
  request: DeleteRoleRequest,
  csrfToken: string,
): Promise<DeleteRoleResponse> {
  return postRoleJson<DeleteRoleResponse>("/roles/delete", request, csrfToken);
}

export function applyRole(
  request: ApplyRoleRequest,
  csrfToken: string,
): Promise<ApplyRoleResponse> {
  return postRoleJson<ApplyRoleResponse>("/roles/apply", request, csrfToken);
}

export function checkPermission(
  request: PermissionCheckRequest,
): Promise<PermissionCheckResponse> {
  return postRoleJson<PermissionCheckResponse>("/permissions/check", request);
}
