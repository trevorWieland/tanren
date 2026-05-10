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
  PermissionCheckRequest,
  PermissionCheckResponse,
  PermissionName,
  PermissionScope,
  ProjectId,
  RoleAdminCapabilities,
  RoleFailureBody,
  RoleId,
  RolePrincipalRejectionRef,
  RoleReadModelRequest,
  RoleReadModelResponse,
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
  parseRoleCapabilitySnapshot,
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

interface RolePostEndpointMap {
  "/roles": {
    request: CreateRoleRequest;
    response: CreateRoleResponse;
  };
  "/roles/edit": {
    request: EditRoleRequest;
    response: EditRoleResponse;
  };
  "/roles/delete": {
    request: DeleteRoleRequest;
    response: DeleteRoleResponse;
  };
  "/roles/apply": {
    request: ApplyRoleRequest;
    response: ApplyRoleResponse;
  };
  "/permissions/check": {
    request:
      | PermissionCheckRequest
      | PermissionCheckRolePrincipalRejectionRequest;
    response: PermissionCheckResponse;
  };
  "/roles/read-model": {
    request: RoleReadModelRequest;
    response: RoleReadModelResponse;
  };
}

type RolePostPath = keyof RolePostEndpointMap;
type ResponseDecoder<TResponse> = (payload: unknown) => TResponse;

interface PostRoleEndpointOptions {
  signal?: AbortSignal;
  timeoutMs?: number;
}

export const ROLE_READ_MODEL_DEFAULT_ROLE_PAGE_SIZE = 50;
export const ROLE_READ_MODEL_DEFAULT_GRANT_PAGE_SIZE = 50;
export const ROLE_REQUEST_TIMEOUT_MS = 10_000;

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

function assertNever(value: never, context: string): never {
  throw new Error(`${context} received unsupported variant: ${String(value)}`);
}

function toRoleTransportError(
  cause: unknown,
  fallbackSummary: string,
): RoleRequestError {
  return new RoleRequestError(
    toRoleTransportFailure(
      cause instanceof Error ? cause.message : fallbackSummary,
    ),
  );
}

function isAbortError(cause: unknown): boolean {
  return cause instanceof DOMException && cause.name === "AbortError";
}

function toTimeoutError(timeoutMs: number): DOMException {
  return new DOMException(
    `request timed out after ${timeoutMs}ms`,
    "TimeoutError",
  );
}

function composeAbortSignal(
  timeoutMs: number,
  externalSignal?: AbortSignal,
): { signal: AbortSignal; cleanup: () => void } {
  const controller = new AbortController();
  const timeout = setTimeout(
    () => controller.abort(toTimeoutError(timeoutMs)),
    timeoutMs,
  );

  if (externalSignal?.aborted) {
    controller.abort(externalSignal.reason);
  }

  const onExternalAbort = (): void => {
    controller.abort(externalSignal?.reason);
  };
  externalSignal?.addEventListener("abort", onExternalAbort, { once: true });

  return {
    signal: controller.signal,
    cleanup: () => {
      clearTimeout(timeout);
      externalSignal?.removeEventListener("abort", onExternalAbort);
    },
  };
}

function parseRoleFailurePayload(payload: unknown): RoleFailureBody {
  try {
    return parseRoleFailure(payload);
  } catch {
    return toRoleTransportFailure("unexpected failure payload");
  }
}

function toRoleRequestErrorFromFailurePayload(
  payload: unknown,
): RoleRequestError {
  return new RoleRequestError(parseRoleFailurePayload(payload));
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

async function postRoleEndpoint<K extends RolePostPath>(
  path: K,
  body: RolePostEndpointMap[K]["request"],
  csrfToken?: string,
  options?: PostRoleEndpointOptions,
): Promise<RolePostEndpointMap[K]["response"]> {
  const timeoutMs = options?.timeoutMs ?? ROLE_REQUEST_TIMEOUT_MS;
  const timedSignal = composeAbortSignal(timeoutMs, options?.signal);
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
      signal: timedSignal.signal,
    });
  } catch (cause: unknown) {
    timedSignal.cleanup();
    if (isAbortError(cause)) {
      throw cause;
    }
    throw toRoleTransportError(cause, String(cause));
  }
  timedSignal.cleanup();

  let payload: unknown = undefined;
  try {
    payload = await response.json();
  } catch {
    payload = undefined;
  }

  if (!response.ok) {
    throw toRoleRequestErrorFromFailurePayload(payload);
  }

  const decode = ROLE_POST_DECODERS[path];
  try {
    return decode(payload);
  } catch (cause: unknown) {
    throw toRoleTransportError(cause, `unexpected ${path} response payload`);
  }
}

export interface RoleCapabilitySnapshot {
  capabilities: RoleAdminCapabilities;
  csrfToken: string;
}

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

export function formatRoleError(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}

export async function fetchRoleCapabilities(): Promise<RoleCapabilitySnapshot> {
  const timedSignal = composeAbortSignal(ROLE_REQUEST_TIMEOUT_MS);
  let response: Response;
  try {
    response = await fetch(resolveRoleApiPath("/roles/capabilities"), {
      method: "GET",
      cache: "no-store",
      credentials: "include",
      signal: timedSignal.signal,
    });
  } catch (cause: unknown) {
    timedSignal.cleanup();
    throw toRoleTransportError(cause, String(cause));
  }
  timedSignal.cleanup();

  let payload: unknown = undefined;
  try {
    payload = await response.json();
  } catch {
    payload = undefined;
  }

  if (!response.ok) {
    throw toRoleRequestErrorFromFailurePayload(payload);
  }

  try {
    const snapshot = parseRoleCapabilitySnapshot(payload);
    return {
      capabilities: snapshot.capabilities,
      csrfToken: snapshot.csrf_token,
    };
  } catch (cause: unknown) {
    throw toRoleTransportError(cause, "unexpected capability payload");
  }
}

export function createRole(
  request: CreateRoleRequest,
  csrfToken: string,
): Promise<CreateRoleResponse> {
  return postRoleEndpoint("/roles", request, csrfToken);
}

export function editRole(
  request: EditRoleRequest,
  csrfToken: string,
): Promise<EditRoleResponse> {
  return postRoleEndpoint("/roles/edit", request, csrfToken);
}

export function deleteRole(
  request: DeleteRoleRequest,
  csrfToken: string,
): Promise<DeleteRoleResponse> {
  return postRoleEndpoint("/roles/delete", request, csrfToken);
}

export function applyRole(
  request: ApplyRoleRequest,
  csrfToken: string,
): Promise<ApplyRoleResponse> {
  return postRoleEndpoint("/roles/apply", request, csrfToken);
}

export function checkPermission(
  request: PermissionCheckRequest,
  csrfToken: string,
): Promise<PermissionCheckResponse> {
  return postRoleEndpoint("/permissions/check", request, csrfToken);
}

export function checkPermissionRolePrincipalRejection(
  request: PermissionCheckRolePrincipalRejectionRequest,
  csrfToken: string,
): Promise<PermissionCheckResponse> {
  return postRoleEndpoint("/permissions/check", request, csrfToken);
}

export function readRoleModel(
  request: RoleReadModelRequest,
  signal?: AbortSignal,
): Promise<RoleReadModelResponse> {
  return postRoleEndpoint(
    "/roles/read-model",
    request,
    undefined,
    signal === undefined ? undefined : { signal },
  );
}

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
