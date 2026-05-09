import * as m from "@/i18n/paraglide/messages";
import type {
  InterfaceError,
  MyPermissionEntry,
  MyPermissionsResponse,
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
export type {
  InterfaceError,
  MyPermissionEntry,
  MyPermissionsResponse,
  PermissionConstraintView,
  PermissionGrantSource,
};

/**
 * Map an `InterfaceError` to a localized message via paraglide. Falls back
 * to the API-supplied summary, then to a generic "Request failed" string,
 * so unknown failure codes still surface something meaningful.
 */
export function describeFailure(failure: InterfaceError): string {
  const key = `failure_${failure.code}`;
  const lookup = m as unknown as Record<string, (() => string) | undefined>;
  const fn = lookup[key];
  if (typeof fn === "function") {
    return fn();
  }
  if (failure.summary !== "") {
    return failure.summary;
  }
  return m.failure_fallback();
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
      return { code, summary };
    }
  }
  return { code: "internal_error", summary: `HTTP ${fallbackStatus}` };
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
    throw new AccountRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
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
    throw new AccountRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }
  if (!response.ok) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: `HTTP ${response.status}`,
    });
  }
}
