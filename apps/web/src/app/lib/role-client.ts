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
): Promise<TResponse> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
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

export function createRole(
  request: CreateRoleRequest,
): Promise<CreateRoleResponse> {
  return postRoleJson<CreateRoleResponse>("/roles", request);
}

export function editRole(request: EditRoleRequest): Promise<EditRoleResponse> {
  return postRoleJson<EditRoleResponse>("/roles/edit", request);
}

export function deleteRole(
  request: DeleteRoleRequest,
): Promise<DeleteRoleResponse> {
  return postRoleJson<DeleteRoleResponse>("/roles/delete", request);
}

export function applyRole(
  request: ApplyRoleRequest,
): Promise<ApplyRoleResponse> {
  return postRoleJson<ApplyRoleResponse>("/roles/apply", request);
}

export function checkPermission(
  request: PermissionCheckRequest,
): Promise<PermissionCheckResponse> {
  return postRoleJson<PermissionCheckResponse>("/permissions/check", request);
}
