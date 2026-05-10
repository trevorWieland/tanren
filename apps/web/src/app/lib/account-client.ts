import * as m from "@/i18n/paraglide/messages";
import { TANREN_API_BASE_URL } from "@/app/lib/api-base-url";
import type {
  CurrentDeploymentPostureResponse,
  DeploymentPosture,
  DeploymentPostureFailureCode,
  DeploymentPostureScope,
  SetDeploymentPostureRequest,
  SetDeploymentPostureResponse,
  SupportedDeploymentPosture,
  SupportedDeploymentPosturesResponse,
} from "@/app/lib/generated/deployment-posture-contract";
import {
  decodeCurrentDeploymentPostureResponse,
  decodeSetDeploymentPostureResponse,
  decodeSupportedDeploymentPosturesResponse,
  isDeploymentPostureFailureCode,
} from "@/app/lib/generated/deployment-posture-contract";

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
export type {
  CurrentDeploymentPostureResponse as DeploymentPostureGetResponse,
  DeploymentPosture,
  DeploymentPostureScope,
  SetDeploymentPostureRequest,
  SetDeploymentPostureResponse,
  SupportedDeploymentPosture,
  SupportedDeploymentPosturesResponse as DeploymentPostureListResponse,
};

let deploymentPostureListCache: Promise<SupportedDeploymentPosturesResponse> | null =
  null;
const ACTIVE_ACCOUNT_SCOPE_STORAGE_KEY = "tanren.active-account-scope-id";
let activeAccountScopeCache: string | null | undefined;

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

function loadCachedActiveAccountScopeId(): string | null {
  if (activeAccountScopeCache !== undefined) {
    return activeAccountScopeCache;
  }
  if (typeof window === "undefined") {
    activeAccountScopeCache = null;
    return activeAccountScopeCache;
  }
  const raw = window.localStorage.getItem(ACTIVE_ACCOUNT_SCOPE_STORAGE_KEY);
  if (raw === null || raw.trim() === "") {
    activeAccountScopeCache = null;
    return activeAccountScopeCache;
  }
  activeAccountScopeCache = raw.trim();
  return activeAccountScopeCache;
}

function persistActiveAccountScopeId(accountId: string): void {
  const normalized = accountId.trim();
  activeAccountScopeCache = normalized === "" ? null : normalized;
  if (typeof window === "undefined") {
    return;
  }
  if (activeAccountScopeCache === null) {
    window.localStorage.removeItem(ACTIVE_ACCOUNT_SCOPE_STORAGE_KEY);
  } else {
    window.localStorage.setItem(
      ACTIVE_ACCOUNT_SCOPE_STORAGE_KEY,
      activeAccountScopeCache,
    );
  }
}

export function getActiveAccountScopeId(): string | null {
  return loadCachedActiveAccountScopeId();
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
  | DeploymentPostureFailureCode;

export interface AccountFailure {
  code: AccountFailureCode;
  summary: string;
  unknown_code?: string;
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

const CORE_ACCOUNT_FAILURE_CODES = [
  "duplicate_identifier",
  "invalid_credential",
  "invitation_not_found",
  "invitation_already_consumed",
  "invitation_expired",
] as const;

function isCoreAccountFailureCode(
  raw: unknown,
): raw is (typeof CORE_ACCOUNT_FAILURE_CODES)[number] {
  return (
    typeof raw === "string" &&
    CORE_ACCOUNT_FAILURE_CODES.includes(
      raw as (typeof CORE_ACCOUNT_FAILURE_CODES)[number],
    )
  );
}

function normalizeFailureCode(raw: unknown): {
  code: AccountFailureCode;
  unknown_code?: string;
} {
  if (isCoreAccountFailureCode(raw) || isDeploymentPostureFailureCode(raw)) {
    return { code: raw };
  }
  if (typeof raw === "string" && raw !== "") {
    return { code: "internal_error", unknown_code: raw };
  }
  return { code: "internal_error" };
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

async function requestJson<T>(
  method: "GET" | "POST",
  path: string,
  body?: unknown,
  decode?: (raw: unknown) => T,
): Promise<T> {
  let response: Response;
  try {
    const request: RequestInit = {
      method,
      // Cookie transport: send/receive HTTP-only session cookie on every
      // request. Replaces localStorage token storage (M2).
      credentials: "include",
    };
    if (body !== undefined) {
      request.headers = { "content-type": "application/json" };
      request.body = JSON.stringify(body);
    }
    response = await fetch(`${TANREN_API_BASE_URL}${path}`, request);
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
    const code = normalizeFailureCode(parsed.code);
    const summary =
      typeof parsed.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    if (code.unknown_code) {
      throw new AccountRequestError({
        code: code.code,
        summary,
        unknown_code: code.unknown_code,
      });
    }
    throw new AccountRequestError({
      code: code.code,
      summary,
    });
  }

  let payload: unknown;
  try {
    payload = (await response.json()) as unknown;
  } catch {
    throw new AccountRequestError({
      code: "internal_error",
      summary: `Invalid JSON body for ${path}`,
    });
  }

  if (!decode) {
    return payload as T;
  }

  try {
    return decode(payload);
  } catch (cause: unknown) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }
}

async function postJson<T>(path: string, body: unknown): Promise<T> {
  return requestJson<T>("POST", path, body);
}

async function postJsonDecoded<T>(
  path: string,
  body: unknown,
  decode: (raw: unknown) => T,
): Promise<T> {
  return requestJson<T>("POST", path, body, decode);
}

async function getJsonDecoded<T>(
  path: string,
  decode: (raw: unknown) => T,
): Promise<T> {
  return requestJson<T>("GET", path, undefined, decode);
}

export async function signUp(input: SignUpInput): Promise<SignUpResult> {
  const result = await postJson<SignUpResult>("/accounts", input);
  persistActiveAccountScopeId(result.session.account_id);
  return result;
}

export async function signIn(input: SignInInput): Promise<SignInResult> {
  const result = await postJson<SignInResult>("/sessions", input);
  persistActiveAccountScopeId(result.session.account_id);
  return result;
}

export async function acceptInvitation(
  token: string,
  input: Omit<AcceptInvitationInput, "invitation_token">,
): Promise<AcceptInvitationResult> {
  const path = `/invitations/${encodeURIComponent(token)}/accept`;
  const result = await postJson<AcceptInvitationResult>(path, {
    email: input.email,
    password: input.password,
    display_name: input.display_name,
  });
  persistActiveAccountScopeId(result.session.account_id);
  return result;
}

export function listDeploymentPostures(): Promise<SupportedDeploymentPosturesResponse> {
  return getJsonDecoded<SupportedDeploymentPosturesResponse>(
    "/deployment-postures",
    decodeSupportedDeploymentPosturesResponse,
  );
}

export function listDeploymentPosturesCached(): Promise<SupportedDeploymentPosturesResponse> {
  if (deploymentPostureListCache === null) {
    deploymentPostureListCache = listDeploymentPostures().catch((error) => {
      deploymentPostureListCache = null;
      throw error;
    });
  }
  return deploymentPostureListCache;
}

export function getDeploymentPosture(
  scopeKind: "account" | "project" | "installation",
  scopeId: string,
): Promise<CurrentDeploymentPostureResponse> {
  const path = `/deployment-postures/${encodeURIComponent(scopeKind)}/${encodeURIComponent(scopeId)}`;
  return getJsonDecoded<CurrentDeploymentPostureResponse>(
    path,
    decodeCurrentDeploymentPostureResponse,
  );
}

export function setDeploymentPosture(
  request: SetDeploymentPostureRequest,
): Promise<SetDeploymentPostureResponse> {
  return postJsonDecoded<SetDeploymentPostureResponse>(
    "/deployment-postures",
    request,
    decodeSetDeploymentPostureResponse,
  );
}

/**
 * Sign-out clears the session row server-side and the cookie via
 * `Set-Cookie: tanren_session=; Max-Age=0`.
 */
export async function signOut(): Promise<void> {
  let response: Response;
  try {
    response = await fetch(`${TANREN_API_BASE_URL}/sessions/revoke`, {
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
  persistActiveAccountScopeId("");
}
