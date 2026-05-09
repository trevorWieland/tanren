import * as m from "@/i18n/paraglide/messages";
import type {
  InterfaceErrorCode,
  MyPermissionEntry,
  MyPermissionsResponse as GeneratedMyPermissionsResponse,
  PermissionConstraintView,
  PermissionGrantSource,
} from "@/app/lib/generated-interface-contracts";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";
const MY_PERMISSIONS_LIMIT = 100;
const MY_PERMISSIONS_DISCOVERY_LIMIT = 1;

export interface SignUpInput {
  email: string;
  password: string;
  display_name: string;
}

export interface SignInInput {
  email: string;
  password: string;
}

export interface AcceptInvitationInput {
  email: string;
  invitation_token: string;
  password: string;
  display_name: string;
}

export interface AccountView {
  id: string;
  identifier: string;
  display_name: string;
  org: string | null;
}

/**
 * Cookie transport: API sets an HTTP-only cookie via tower-sessions on
 * sign-up/sign-in/accept-invitation. The body carries metadata only —
 * the session token itself is never readable from JavaScript.
 */
export interface SessionView {
  account_id: string;
  expires_at: string;
}

export interface SignUpResult {
  account: AccountView;
  session: SessionView;
}

export interface SignInResult {
  account: AccountView;
  session: SessionView;
}

export interface AcceptInvitationResult {
  account: AccountView;
  session: SessionView;
  joined_org: string;
}
export interface InterfaceError {
  code: InterfaceErrorCode;
  summary: string;
}
export type MyPermissionsResponse = GeneratedMyPermissionsResponse;
export type PermissionScopeView =
  | {
      kind: "organization";
      scope_id: string;
      permissions: MyPermissionEntry[];
    }
  | {
      kind: "project";
      scope_id: string;
      permissions: MyPermissionEntry[];
    };
export type {
  MyPermissionEntry,
  PermissionConstraintView,
  PermissionGrantSource,
};

function scopeLabel(scope: PermissionScopeView): string {
  switch (scope.kind) {
    case "organization":
      return scope.scope_id;
    case "project":
      return scope.scope_id;
  }
}

export function permissionScopes(
  response: MyPermissionsResponse,
): PermissionScopeView[] {
  const organizations: PermissionScopeView[] = response.organizations.map(
    (section) => ({
      kind: "organization",
      scope_id: section.org_id,
      permissions: section.permissions,
    }),
  );
  const projects: PermissionScopeView[] = response.projects.map((section) => ({
    kind: "project",
    scope_id: section.project_id,
    permissions: section.permissions,
  }));
  return [...organizations, ...projects].sort((left, right) =>
    scopeLabel(left).localeCompare(scopeLabel(right)),
  );
}

/**
 * Map an `InterfaceError` to a localized message via paraglide. Falls back
 * to the API-supplied summary, then to a generic "Request failed" string,
 * so unknown failure codes still surface something meaningful.
 */
export function describeFailure(failure: InterfaceError): string {
  switch (failure.code) {
    case "duplicate_identifier":
      return m.failure_duplicate_identifier();
    case "invalid_credential":
      return m.failure_invalid_credential();
    case "validation_failed":
      return m.failure_validation_failed();
    case "invitation_not_found":
      return m.failure_invitation_not_found();
    case "invitation_already_consumed":
      return m.failure_invitation_already_consumed();
    case "invitation_expired":
      return m.failure_invitation_expired();
    case "auth_required":
      return m.failure_auth_required();
    case "permission_denied":
      return m.failure_permission_denied();
    case "unavailable":
      return m.failure_unavailable();
    case "internal_error":
      return m.failure_internal_error();
    case "not_found":
    case "conflict":
    case "idempotency_conflict":
    case "stale_projection":
    case "drift_detected":
    case "rate_limited":
    case "unsupported_action":
    case "provider_failure":
    case "execution_failure":
      if (failure.summary !== "") {
        return failure.summary;
      }
      return m.failure_fallback();
  }
}

export class AccountRequestError extends Error {
  readonly failure: InterfaceError;

  constructor(failure: InterfaceError) {
    super(describeFailure(failure));
    this.failure = failure;
    this.name = "AccountRequestError";
  }
}

function normalizeInterfaceError(
  payload: unknown,
  fallbackStatus: number,
): InterfaceError {
  if (
    typeof payload === "object" &&
    payload !== null &&
    "code" in payload &&
    "summary" in payload
  ) {
    const code = (payload as { code: unknown }).code;
    const summary = (payload as { summary: unknown }).summary;
    if (typeof code === "string" && typeof summary === "string") {
      const normalizedCode = parseInterfaceErrorCode(code);
      if (normalizedCode !== null) {
        return { code: normalizedCode, summary };
      }
      return { code: "internal_error", summary };
    }
  }
  return {
    code: "internal_error",
    summary: fallbackStatus > 0 ? `HTTP ${fallbackStatus}` : "",
  };
}

function parseInterfaceErrorCode(raw: string): InterfaceErrorCode | null {
  switch (raw) {
    case "auth_required":
    case "permission_denied":
    case "validation_failed":
    case "not_found":
    case "conflict":
    case "idempotency_conflict":
    case "stale_projection":
    case "drift_detected":
    case "rate_limited":
    case "unavailable":
    case "unsupported_action":
    case "provider_failure":
    case "execution_failure":
    case "internal_error":
    case "duplicate_identifier":
    case "invalid_credential":
    case "invitation_not_found":
    case "invitation_expired":
    case "invitation_already_consumed":
      return raw;
    default:
      return null;
  }
}

async function requestJson<T>(
  path: string,
  method: "GET" | "POST",
  body?: unknown,
): Promise<T> {
  const requestInit: RequestInit =
    method === "POST"
      ? {
          method,
          headers: { "content-type": "application/json" },
          body: JSON.stringify(body),
          credentials: "include",
        }
      : {
          method,
          credentials: "include",
        };
  let response: Response;
  try {
    // Cookie transport: send/receive HTTP-only session cookie on every
    // request. Replaces localStorage token storage (M2).
    response = await fetch(`${API_URL}${path}`, requestInit);
  } catch (cause: unknown) {
    const failure: InterfaceError = {
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    };
    throw new AccountRequestError(failure);
  }

  if (!response.ok) {
    let parsed: unknown = null;
    try {
      parsed = await response.json();
    } catch {
      parsed = null;
    }
    throw new AccountRequestError(
      normalizeInterfaceError(parsed, response.status),
    );
  }

  return (await response.json()) as T;
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  return requestJson<T>(path, "POST", body);
}

export function signUp(input: SignUpInput): Promise<SignUpResult> {
  return postJson<SignUpResult>("/accounts", input);
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  return postJson<SignInResult>("/sessions", input);
}

export function acceptInvitation(
  token: string,
  input: Omit<AcceptInvitationInput, "invitation_token">,
): Promise<AcceptInvitationResult> {
  const path = `/invitations/${encodeURIComponent(token)}/accept`;
  return postJson<AcceptInvitationResult>(path, {
    email: input.email,
    password: input.password,
    display_name: input.display_name,
  });
}

export function myPermissions(): Promise<MyPermissionsResponse> {
  return requestJson<MyPermissionsResponse>(
    `/me/permissions?limit=${MY_PERMISSIONS_LIMIT}`,
    "GET",
  );
}

export async function canAccessMyPermissions(): Promise<boolean> {
  try {
    await requestJson<MyPermissionsResponse>(
      `/me/permissions?limit=${MY_PERMISSIONS_DISCOVERY_LIMIT}`,
      "GET",
    );
    return true;
  } catch (cause: unknown) {
    if (cause instanceof AccountRequestError) {
      return false;
    }
    return false;
  }
}

/**
 * Sign-out clears the session row server-side and the cookie via
 * `Set-Cookie: tanren_session=; Max-Age=0`.
 */
export async function signOut(): Promise<void> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}/sessions/revoke`, {
      method: "POST",
      credentials: "include",
    });
  } catch (cause: unknown) {
    const failure: InterfaceError = {
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    };
    throw new AccountRequestError(failure);
  }
  if (!response.ok) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: `HTTP ${response.status}`,
    });
  }
}
