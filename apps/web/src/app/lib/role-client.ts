import type {
  AccountId,
  AccountPrincipalRef,
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
  RolePrincipalRejectionRef,
  RoleReadModelRequest,
  RoleReadModelResponse,
  PermissionScope,
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

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "";
const ALLOWLISTED_LOOPBACK_API_HOSTS = new Set([
  "127.0.0.1",
  "localhost",
  "::1",
]);

export class RoleRequestError extends Error {
  readonly failure: RoleFailureBody;

  constructor(failure: RoleFailureBody) {
    super(`${failure.code}: ${failure.summary}`);
    this.failure = failure;
    this.name = "RoleRequestError";
  }
}

export type ScopeKind = RoleScope["scope"];
export type PrincipalKind =
  | AccountPrincipalRef["principal"]
  | RolePrincipalRejectionRef["principal"];

export interface PermissionCheckRolePrincipalRejectionRequest {
  principal: RolePrincipalRejectionRef;
  permission: PermissionName;
  scope: PermissionScope;
}

type RoleCommandRequest =
  | CreateRoleRequest
  | EditRoleRequest
  | DeleteRoleRequest
  | ApplyRoleRequest
  | PermissionCheckRequest
  | PermissionCheckRolePrincipalRejectionRequest
  | RoleReadModelRequest;

type ResponseDecoder<TResponse> = (payload: unknown) => TResponse;

function toRoleTransportFailure(summary: string): RoleFailureBody {
  return {
    code: "transport_error",
    summary,
  };
}

function toRoleValidationFailure(summary: string): RoleFailureBody {
  return {
    code: "validation_failed",
    summary,
  };
}

function toRoleValidationError(summary: string): RoleRequestError {
  return new RoleRequestError(toRoleValidationFailure(summary));
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function normalizeAbsoluteApiBase(url: URL): string {
  if (url.search.length > 0 || url.hash.length > 0) {
    throw toRoleValidationError(
      "NEXT_PUBLIC_API_URL must not include query params or fragments",
    );
  }
  const pathname = url.pathname === "/" ? "" : trimTrailingSlash(url.pathname);
  return `${url.origin}${pathname}`;
}

function isAllowlistedAbsoluteApiOrigin(url: URL): boolean {
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return false;
  }
  if (
    typeof window !== "undefined" &&
    url.origin.toLowerCase() === window.location.origin.toLowerCase()
  ) {
    return true;
  }
  return ALLOWLISTED_LOOPBACK_API_HOSTS.has(url.hostname.toLowerCase());
}

function resolveApiBasePath(): string {
  const trimmed = API_URL.trim();
  if (trimmed.length === 0) {
    return "";
  }
  if (trimmed.startsWith("/")) {
    return trimTrailingSlash(trimmed);
  }

  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    throw toRoleValidationError(
      "NEXT_PUBLIC_API_URL must be relative or an absolute HTTP(S) URL",
    );
  }

  if (!isAllowlistedAbsoluteApiOrigin(parsed)) {
    throw toRoleValidationError(
      `NEXT_PUBLIC_API_URL origin is not allowlisted: ${parsed.origin}`,
    );
  }

  return normalizeAbsoluteApiBase(parsed);
}

function resolveRoleApiPath(path: string): string {
  return `${resolveApiBasePath()}${path}`;
}

async function postRoleJson<TResponse>(
  path: string,
  body: RoleCommandRequest,
  decode: ResponseDecoder<TResponse>,
  csrfToken?: string,
): Promise<TResponse> {
  let response: Response;
  try {
    response = await fetch(resolveRoleApiPath(path), {
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
    response = await fetch(resolveRoleApiPath("/roles/capabilities"), {
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

export function checkPermissionRolePrincipalRejection(
  request: PermissionCheckRolePrincipalRejectionRequest,
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
  return parts.map((part) => asPermissionName(part));
}

export function readRoleScope(form: FormData, prefix: string): RoleScope {
  const kind = parseScopeKind(readNonEmptyField(form, `${prefix}kind`));
  const id = readNonEmptyField(form, `${prefix}id`);
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
  if (kind !== "account") {
    throw toRoleValidationError(
      `field ${prefix}kind must be account for this request`,
    );
  }
  return {
    principal: "account",
    account_id: asAccountIdValue(readNonEmptyField(form, `${prefix}id`)),
  };
}

export function readRolePrincipalRejectionRef(
  form: FormData,
  prefix: string,
): RolePrincipalRejectionRef {
  const kind = readPrincipalKindField(form, prefix);
  if (kind !== "role") {
    throw toRoleValidationError(
      `field ${prefix}kind must be role for rejection witness requests`,
    );
  }
  return {
    principal: "role",
    role_id: asRoleIdValue(readNonEmptyField(form, `${prefix}id`)),
  };
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

function parseScopeKind(value: string): ScopeKind {
  try {
    return parseRoleScopeKind(value);
  } catch {
    throw toRoleValidationError(
      `scope kind must be account, organization, or project: ${value}`,
    );
  }
}

function parseFormPrincipalKind(value: string): PrincipalKind {
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
