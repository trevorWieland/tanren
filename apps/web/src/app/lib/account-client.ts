import * as m from "@/i18n/paraglide/messages";
import { withJsonContentType } from "@/app/lib/http";

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

export interface HealthReport {
  status: string;
  version: string;
  contract_version: number;
}

export type UserSettingKey = "theme" | "editor";
export type UserCredentialKind = "provider_api_token" | "harness_api_token";

export type UserSettingValue =
  | { kind: "theme"; value: "system" | "light" | "dark" }
  | { kind: "editor"; value: string };

export interface UserSettingView {
  key: UserSettingKey;
  value: UserSettingValue;
  updated_at: string;
}

export interface UserCredentialView {
  id: string;
  kind: UserCredentialKind;
  owner_scope: { scope: "user"; account_id: string };
  status: "pending" | "active" | "invalid";
  created_at: string;
  updated_at: string;
}

export interface UpsertUserSettingInput {
  key: UserSettingKey;
  value: UserSettingValue;
}

export interface UpsertUserSettingResult {
  setting: UserSettingView;
}

export interface ListUserSettingsResult {
  items: UserSettingView[];
}

export interface RemoveUserSettingResult {
  setting: UserSettingView;
}

export interface CreateUserCredentialInput {
  kind: UserCredentialKind;
  value: string;
}

export interface CreateUserCredentialResult {
  item: UserCredentialView;
}

export interface UpdateUserCredentialInput {
  value: string;
}

export interface UpdateUserCredentialResult {
  item: UserCredentialView;
}

export interface ListUserCredentialsResult {
  items: UserCredentialView[];
}

export interface RemoveUserCredentialResult {
  item: UserCredentialView;
}

/**
 * Stable wire codes from `AccountFailureReason` and user-configuration
 * failures in `tanren-contract`.
 */
export type AccountFailureCode =
  | "auth_required"
  | "duplicate_identifier"
  | "internal_error"
  | "invalid_credential"
  | "invitation_already_consumed"
  | "invitation_expired"
  | "invitation_not_found"
  | "item_not_found"
  | "setting_not_found"
  | "unavailable"
  | "validation_failed";

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

async function request(path: string, init: RequestInit): Promise<Response> {
  try {
    return await fetch(`${API_URL}${path}`, {
      ...init,
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new AccountRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }
}

async function parseFailure(response: Response): Promise<AccountFailure> {
  let parsed: FailureBody = {};
  try {
    parsed = (await response.json()) as FailureBody;
  } catch {
    parsed = {};
  }
  const code = typeof parsed.code === "string" ? parsed.code : "internal_error";
  const summary =
    typeof parsed.summary === "string"
      ? parsed.summary
      : `HTTP ${response.status}`;
  return { code, summary };
}

async function requestJson<T>(
  path: string,
  init: RequestInit,
  expectedStatus: readonly number[] = [200],
): Promise<T> {
  const response = await request(path, init);
  if (!expectedStatus.includes(response.status)) {
    throw new AccountRequestError(await parseFailure(response));
  }
  return (await response.json()) as T;
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  return requestJson<T>(
    path,
    {
      method: "POST",
      headers: withJsonContentType(),
      body: JSON.stringify(body),
    },
    [200, 201],
  );
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

export function fetchHealth(): Promise<HealthReport> {
  return requestJson<HealthReport>("/health", { method: "GET" });
}

export function listUserSettings(): Promise<ListUserSettingsResult> {
  return requestJson<ListUserSettingsResult>(
    "/configuration/account/user-settings",
    { method: "GET" },
  );
}

export function upsertUserSetting(
  input: UpsertUserSettingInput,
): Promise<UpsertUserSettingResult> {
  return requestJson<UpsertUserSettingResult>(
    "/configuration/account/user-settings",
    {
      method: "POST",
      headers: withJsonContentType(),
      body: JSON.stringify(input),
    },
  );
}

export function removeUserSetting(
  key: UserSettingKey,
): Promise<RemoveUserSettingResult> {
  return requestJson<RemoveUserSettingResult>(
    `/configuration/account/user-settings/${encodeURIComponent(key)}`,
    { method: "DELETE" },
  );
}

export function listUserCredentials(): Promise<ListUserCredentialsResult> {
  return requestJson<ListUserCredentialsResult>(
    "/configuration/account/user-credentials",
    { method: "GET" },
  );
}

export function addUserCredential(
  input: CreateUserCredentialInput,
): Promise<CreateUserCredentialResult> {
  return requestJson<CreateUserCredentialResult>(
    "/configuration/account/user-credentials",
    {
      method: "POST",
      headers: withJsonContentType(),
      body: JSON.stringify(input),
    },
    [201],
  );
}

export function updateUserCredential(
  itemId: string,
  input: UpdateUserCredentialInput,
): Promise<UpdateUserCredentialResult> {
  return requestJson<UpdateUserCredentialResult>(
    `/configuration/account/user-credentials/${encodeURIComponent(itemId)}`,
    {
      method: "PUT",
      headers: withJsonContentType(),
      body: JSON.stringify(input),
    },
  );
}

export function removeUserCredential(
  itemId: string,
): Promise<RemoveUserCredentialResult> {
  return requestJson<RemoveUserCredentialResult>(
    `/configuration/account/user-credentials/${encodeURIComponent(itemId)}`,
    { method: "DELETE" },
  );
}

/**
 * Sign-out clears the session row server-side and the cookie via
 * `Set-Cookie: tanren_session=; Max-Age=0`.
 */
export async function signOut(): Promise<void> {
  const response = await request("/sessions/revoke", {
    method: "POST",
  });
  if (response.status !== 204) {
    throw new AccountRequestError(await parseFailure(response));
  }
}
