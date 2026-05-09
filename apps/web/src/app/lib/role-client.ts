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
  PermissionScope,
  PrincipalRef,
  RoleAdminCapabilities,
  RoleFailureBody,
  RoleScope,
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

export type ScopeKind = RoleScope["scope"];
export type PrincipalKind = PrincipalRef["principal"];

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

export function formatRoleError(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
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

export function readRequiredField(form: FormData, name: string): string {
  const value = form.get(name);
  if (typeof value !== "string") {
    return "";
  }
  return value.trim();
}

export function readPermissionBundle(form: FormData, name: string): string[] {
  return readRequiredField(form, name)
    .split(",")
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
}

export function readRoleScope(form: FormData, prefix: string): RoleScope {
  const kind = readRequiredField(form, `${prefix}kind`) as ScopeKind;
  const id = readRequiredField(form, `${prefix}id`);
  if (kind === "organization") {
    return { scope: "organization", org_id: id };
  }
  if (kind === "project") {
    return { scope: "project", project_id: id };
  }
  return { scope: "account", account_id: id };
}

export function readPermissionScope(
  form: FormData,
  prefix: string,
): PermissionScope {
  const scope = readRoleScope(form, prefix);
  if (scope.scope === "organization") {
    return { scope: "organization", org_id: scope.org_id };
  }
  if (scope.scope === "project") {
    return { scope: "project", project_id: scope.project_id };
  }
  return { scope: "account", account_id: scope.account_id };
}

export function readPrincipalRef(form: FormData, prefix: string): PrincipalRef {
  const kind = readRequiredField(form, `${prefix}kind`) as PrincipalKind;
  const id = readRequiredField(form, `${prefix}id`);
  if (kind === "role") {
    return { principal: "role", role_id: id };
  }
  return { principal: "account", account_id: id };
}
