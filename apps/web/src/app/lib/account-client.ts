import * as m from "@/i18n/paraglide/messages";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

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

/**
 * Stable wire codes from `AccountFailureReason` in `tanren-contract`.
 * Kept in lock-step with the Rust enum so BDD web steps can match on the
 * same taxonomy regardless of transport.
 */
export type AccountFailureCode =
  | "duplicate_identifier"
  | "invalid_credential"
  | "invitation_not_found"
  | "invitation_already_consumed"
  | "invitation_expired"
  | "validation_failed"
  | "unavailable"
  | "internal_error";

export interface AccountFailure {
  code: AccountFailureCode | string;
  summary: string;
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

/**
 * Map an `AccountFailure` to a localized message via paraglide. Falls back
 * to the API-supplied summary, then to a generic "Request failed" string,
 * so unknown failure codes still surface something meaningful.
 */
export function describeFailure(failure: AccountFailure): string {
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
  readonly failure: AccountFailure;

  constructor(failure: AccountFailure) {
    super(describeFailure(failure));
    this.failure = failure;
    this.name = "AccountRequestError";
  }
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      // Cookie transport: send/receive HTTP-only session cookie on every
      // request. Replaces localStorage token storage (M2).
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new AccountRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  if (!response.ok) {
    let parsed: FailureBody = {};
    try {
      parsed = (await response.json()) as FailureBody;
    } catch {
      parsed = {};
    }
    const code =
      typeof parsed.code === "string" ? parsed.code : "internal_error";
    const summary =
      typeof parsed.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    throw new AccountRequestError({ code, summary });
  }

  return (await response.json()) as T;
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

// ---------------------------------------------------------------------------
// User-tier configuration
// ---------------------------------------------------------------------------

export type UserSettingKey = "preferred_harness" | "preferred_provider";

export interface UserConfigEntry {
  key: UserSettingKey;
  value: string;
  updated_at: string;
}

export interface ListUserConfigResponse {
  entries: UserConfigEntry[];
}

export interface SetUserConfigResponse {
  entry: UserConfigEntry;
}

export interface RemoveResult {
  removed: boolean;
}

export function listUserConfig(): Promise<ListUserConfigResponse> {
  return getJson<ListUserConfigResponse>("/me/config");
}

export function setUserConfig(
  key: UserSettingKey,
  value: string,
): Promise<SetUserConfigResponse> {
  return postJson<SetUserConfigResponse>("/me/config", { key, value });
}

export function removeUserConfig(key: UserSettingKey): Promise<RemoveResult> {
  return deleteJson<RemoveResult>(`/me/config/${encodeURIComponent(key)}`);
}

// ---------------------------------------------------------------------------
// User-owned credentials
// ---------------------------------------------------------------------------

export type CredentialKind =
  | "api_key"
  | "source_control_token"
  | "webhook_signing_key"
  | "oidc_client_secret"
  | "opaque_secret";

export interface RedactedCredentialMetadata {
  id: string;
  name: string;
  kind: CredentialKind;
  scope: string;
  description: string | null;
  provider: string | null;
  created_at: string;
  updated_at: string | null;
  present: boolean;
}

export interface ListCredentialsResponse {
  credentials: RedactedCredentialMetadata[];
}

export interface CreateCredentialInput {
  kind: CredentialKind;
  name: string;
  description?: string;
  provider?: string;
  value: string;
}

export interface CreateCredentialResponse {
  credential: RedactedCredentialMetadata;
}

export interface UpdateCredentialInput {
  name?: string;
  description?: string;
  value: string;
}

export interface UpdateCredentialResponse {
  credential: RedactedCredentialMetadata;
}

export function listCredentials(): Promise<ListCredentialsResponse> {
  return getJson<ListCredentialsResponse>("/me/credentials");
}

export function createCredential(
  input: CreateCredentialInput,
): Promise<CreateCredentialResponse> {
  return postJson<CreateCredentialResponse>("/me/credentials", input);
}

export function updateCredential(
  id: string,
  input: UpdateCredentialInput,
): Promise<UpdateCredentialResponse> {
  return patchJson<UpdateCredentialResponse>(
    `/me/credentials/${encodeURIComponent(id)}`,
    input,
  );
}

export function removeCredential(id: string): Promise<RemoveResult> {
  return deleteJson<RemoveResult>(`/me/credentials/${encodeURIComponent(id)}`);
}

// ---------------------------------------------------------------------------
// Generic authenticated fetch helpers
// ---------------------------------------------------------------------------

async function getJson<T>(path: string): Promise<T> {
  return requestJson<T>(path, { method: "GET" });
}

async function patchJson<T>(path: string, body: unknown): Promise<T> {
  return requestJson<T>(path, {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function deleteJson<T>(path: string): Promise<T> {
  return requestJson<T>(path, { method: "DELETE" });
}

async function requestJson<T>(path: string, init: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_URL}${path}`, {
      ...init,
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new AccountRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  if (!response.ok) {
    let parsed: FailureBody = {};
    try {
      parsed = (await response.json()) as FailureBody;
    } catch {
      parsed = {};
    }
    const code =
      typeof parsed.code === "string" ? parsed.code : "internal_error";
    const summary =
      typeof parsed.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    throw new AccountRequestError({ code, summary });
  }

  return (await response.json()) as T;
}
