import type {
  AccountId,
  ApplyRoleRequest,
  ApplyRoleResponse,
  CreateRoleRequest,
  CreateRoleResponse,
  DeleteRoleRequest,
  DeleteRoleResponse,
  EditRoleRequest,
  EditRoleResponse,
  OrgId,
  PermissionName,
  ProjectId,
  PermissionCheckRequest,
  PermissionCheckResponse,
  RoleId,
  RoleReadModelRequest,
  RoleReadModelResponse,
  PermissionScope,
  PrincipalRef,
  RoleAdminCapabilities,
  RoleFailureBody,
  RoleScope,
} from "./generated/role-contract";
import {
  asAccountId,
  asOrgId,
  asPermissionName,
  asProjectId,
  asRoleId,
  parseApplyRoleResponse,
  parseCreateRoleResponse,
  parseDeleteRoleResponse,
  parseEditRoleResponse,
  parsePermissionCheckResponse,
  parsePrincipalKind,
  parseRoleCapabilitySnapshotPayload,
  parseRoleFailure,
  parseRoleReadModelResponse,
  parseRoleScopeKind,
} from "./generated/role-contract";

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
  | PermissionCheckRequest
  | RoleReadModelRequest;

type ResponseDecoder<TResponse> = (payload: unknown) => TResponse;

function toRoleTransportFailure(summary: string): RoleFailureBody {
  return {
    code: "transport_error",
    summary,
  };
}

async function postRoleJson<TResponse>(
  path: string,
  body: RoleCommandRequest,
  decode: ResponseDecoder<TResponse>,
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
    throw new RoleRequestError(
      toRoleTransportFailure(
        cause instanceof Error ? cause.message : String(cause),
      ),
    );
  }

  let payload: unknown = undefined;
  try {
    payload = await response.json();
  } catch {
    payload = undefined;
  }

  if (!response.ok) {
    throw new RoleRequestError(parseRoleFailure(payload));
  }

  try {
    return decode(payload);
  } catch (cause: unknown) {
    throw new RoleRequestError(
      toRoleTransportFailure(
        cause instanceof Error
          ? cause.message
          : `unexpected ${path} response payload`,
      ),
    );
  }
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
    throw new RoleRequestError(
      toRoleTransportFailure(
        cause instanceof Error ? cause.message : String(cause),
      ),
    );
  }

  let payload: unknown = undefined;
  try {
    payload = await response.json();
  } catch {
    payload = undefined;
  }

  if (!response.ok) {
    throw new RoleRequestError(parseRoleFailure(payload));
  }

  try {
    const typed = parseRoleCapabilitySnapshotPayload(payload);
    if (typed.csrf_token.length === 0) {
      throw new Error("missing csrf token in capability payload");
    }
    return {
      capabilities: typed.capabilities,
      csrfToken: typed.csrf_token,
    };
  } catch (cause: unknown) {
    throw new RoleRequestError(
      toRoleTransportFailure(
        cause instanceof Error
          ? cause.message
          : "unexpected capability payload",
      ),
    );
  }
}

export function createRole(
  request: CreateRoleRequest,
  csrfToken: string,
): Promise<CreateRoleResponse> {
  return postRoleJson("/roles", request, parseCreateRoleResponse, csrfToken);
}

export function editRole(
  request: EditRoleRequest,
  csrfToken: string,
): Promise<EditRoleResponse> {
  return postRoleJson("/roles/edit", request, parseEditRoleResponse, csrfToken);
}

export function deleteRole(
  request: DeleteRoleRequest,
  csrfToken: string,
): Promise<DeleteRoleResponse> {
  return postRoleJson(
    "/roles/delete",
    request,
    parseDeleteRoleResponse,
    csrfToken,
  );
}

export function applyRole(
  request: ApplyRoleRequest,
  csrfToken: string,
): Promise<ApplyRoleResponse> {
  return postRoleJson(
    "/roles/apply",
    request,
    parseApplyRoleResponse,
    csrfToken,
  );
}

export function checkPermission(
  request: PermissionCheckRequest,
): Promise<PermissionCheckResponse> {
  return postRoleJson(
    "/permissions/check",
    request,
    parsePermissionCheckResponse,
  );
}

export function readRoleModel(
  request: RoleReadModelRequest,
): Promise<RoleReadModelResponse> {
  return postRoleJson("/roles/read-model", request, parseRoleReadModelResponse);
}

export function readRequiredField(form: FormData, name: string): string {
  const value = form.get(name);
  if (typeof value !== "string") {
    return "";
  }
  return value.trim();
}

export function readPermissionBundle(
  form: FormData,
  name: string,
): PermissionName[] {
  return readRequiredField(form, name)
    .split(",")
    .map((part) => asPermissionName(part.trim()))
    .filter((part) => part.length > 0);
}

export function readRoleScope(form: FormData, prefix: string): RoleScope {
  const kind = parseScopeKind(readRequiredField(form, `${prefix}kind`));
  const id = readRequiredField(form, `${prefix}id`);
  if (kind === "organization") {
    return { scope: "organization", org_id: asOrgIdValue(id) };
  }
  if (kind === "project") {
    return { scope: "project", project_id: asProjectIdValue(id) };
  }
  return { scope: "account", account_id: asAccountIdValue(id) };
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
  const kind = parseFormPrincipalKind(readRequiredField(form, `${prefix}kind`));
  const id = readRequiredField(form, `${prefix}id`);
  if (kind === "role") {
    return { principal: "role", role_id: asRoleIdValue(id) };
  }
  return { principal: "account", account_id: asAccountIdValue(id) };
}

export function readRoleIdField(form: FormData, name: string): RoleId {
  return asRoleIdValue(readRequiredField(form, name));
}

export function readPermissionNameField(
  form: FormData,
  name: string,
): PermissionName {
  return asPermissionName(readRequiredField(form, name));
}

function parseScopeKind(value: string): ScopeKind {
  return parseRoleScopeKind(value);
}

function parseFormPrincipalKind(value: string): PrincipalKind {
  return parsePrincipalKind(value);
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
