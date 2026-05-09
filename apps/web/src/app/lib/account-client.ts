import * as m from "@/i18n/paraglide/messages";
import { withJsonContentType } from "@/app/lib/http";
import type {
  AccountFailure,
  ConfigurationCapabilities,
  ConfigurationDiscoveryResult,
  CreateUserCredentialInput,
  CreateUserCredentialResult,
  GetConfigurationCapabilitiesResult,
  ListUserCredentialsResult,
  ListUserSettingsResult,
  RemoveUserCredentialResult,
  RemoveUserSettingResult,
  UpdateUserCredentialInput,
  UpdateUserCredentialResult,
  UpsertUserSettingInput,
  UpsertUserSettingResult,
  UserCredentialView,
  UserSettingKey,
  UserSettingView,
} from "@/app/lib/api-contracts";

export type {
  AccountFailure,
  ConfigurationCapabilities,
  ConfigurationDiscoveryResult,
  CreateUserCredentialInput,
  CreateUserCredentialResult,
  GetConfigurationCapabilitiesResult,
  ListUserCredentialsResult,
  ListUserSettingsResult,
  RemoveUserCredentialResult,
  RemoveUserSettingResult,
  UpdateUserCredentialInput,
  UpdateUserCredentialResult,
  UpsertUserSettingInput,
  UpsertUserSettingResult,
  UserCredentialKind,
  UserCredentialView,
  UserSettingKey,
  UserSettingValue,
  UserSettingView,
} from "@/app/lib/api-contracts";

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

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

export interface CursorListInput {
  limit?: number;
  after?: string;
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
  return listUserSettingsPage();
}

export function listUserSettingsPage(
  input: CursorListInput = {},
): Promise<ListUserSettingsResult> {
  const params = new URLSearchParams();
  if (typeof input.limit === "number") {
    params.set("limit", String(input.limit));
  }
  if (typeof input.after === "string" && input.after !== "") {
    params.set("after", input.after);
  }
  const suffix = params.toString();
  return requestJson<ListUserSettingsResult>(
    `/configuration/account/user-settings${suffix === "" ? "" : `?${suffix}`}`,
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
  return listUserCredentialsPage();
}

export function listUserCredentialsPage(
  input: CursorListInput = {},
): Promise<ListUserCredentialsResult> {
  const params = new URLSearchParams();
  if (typeof input.limit === "number") {
    params.set("limit", String(input.limit));
  }
  if (typeof input.after === "string" && input.after !== "") {
    params.set("after", input.after);
  }
  const suffix = params.toString();
  return requestJson<ListUserCredentialsResult>(
    `/configuration/account/user-credentials${suffix === "" ? "" : `?${suffix}`}`,
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

export function getConfigurationCapabilities(): Promise<GetConfigurationCapabilitiesResult> {
  return requestJson<GetConfigurationCapabilitiesResult>(
    "/configuration/account/capabilities",
    { method: "GET" },
  );
}

function unavailableFailure(summary: string): AccountFailure {
  return {
    code: "unavailable",
    summary,
  };
}

function failureFromUnknown(error: unknown): AccountFailure {
  if (error instanceof AccountRequestError) {
    return error.failure;
  }
  if (error instanceof Error) {
    return unavailableFailure(error.message);
  }
  return unavailableFailure(String(error));
}

function deniedCapabilities(): ConfigurationCapabilities {
  return {
    settings: {
      allowed_actions: [],
    },
    user_items: {
      allowed_actions: [],
    },
  };
}

/**
 * Capability discovery for configuration controls. Mutating actions are gated
 * by the explicit capability contract, not inferred from read success.
 */
export async function discoverConfigurationAccess(): Promise<ConfigurationDiscoveryResult> {
  const [capabilitiesResult, settingsResult, credentialsResult] =
    await Promise.allSettled([
      getConfigurationCapabilities(),
      listUserSettings(),
      listUserCredentials(),
    ]);

  const settings: UserSettingView[] =
    settingsResult.status === "fulfilled" ? settingsResult.value.items : [];
  const credentials: UserCredentialView[] =
    credentialsResult.status === "fulfilled"
      ? credentialsResult.value.items
      : [];

  const settingsFailure =
    settingsResult.status === "rejected"
      ? failureFromUnknown(settingsResult.reason)
      : null;
  const credentialsFailure =
    credentialsResult.status === "rejected"
      ? failureFromUnknown(credentialsResult.reason)
      : null;

  const capabilitiesFailure =
    capabilitiesResult.status === "rejected"
      ? failureFromUnknown(capabilitiesResult.reason)
      : null;
  const capabilities =
    capabilitiesResult.status === "fulfilled"
      ? capabilitiesResult.value.capabilities
      : deniedCapabilities();

  return {
    capabilities,
    settings,
    credentials,
    capabilities_failure: capabilitiesFailure,
    settings_failure: settingsFailure,
    credentials_failure: credentialsFailure,
  };
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
