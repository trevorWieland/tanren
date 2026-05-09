import * as m from "@/i18n/paraglide/messages";
import type {
  AccountFailureCode,
  AccountId,
  AccountView,
  ListActiveAccountsResponse,
  OrgId,
  SignedInAccountView,
  SwitchActiveAccountRequest,
  SwitchActiveAccountResponse,
} from "@/app/lib/generated/account-contract";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";
const WINDOW_ID_HEADER = "x-tanren-window-id";
const WINDOW_ID_STORAGE_KEY = "tanren.window_id";

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

export type { AccountFailureCode, AccountView, SignedInAccountView };

/**
 * Cookie transport: API sets an HTTP-only cookie via tower-sessions on
 * sign-up/sign-in/accept-invitation. The body carries metadata only —
 * the session token itself is never readable from JavaScript.
 */
export interface SessionView {
  account_id: AccountId;
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
  joined_org: OrgId;
}

export type ListActiveAccountsResult = ListActiveAccountsResponse;
export type SwitchActiveAccountInput = SwitchActiveAccountRequest;
export type SwitchActiveAccountResult = SwitchActiveAccountResponse;

/**
 * Network/runtime-only extensions layered on top of canonical
 * `AccountFailureCode` from the generated contract.
 */
export type AccountRequestFailureCode =
  | AccountFailureCode
  | "unavailable"
  | "internal_error";

export interface AccountFailure {
  code: AccountRequestFailureCode | string;
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

async function requestJson<T>(
  path: string,
  method: "GET" | "POST",
  body?: unknown,
): Promise<T> {
  let response: Response;
  try {
    const init: RequestInit = {
      method,
      headers: {
        ...(method === "POST" ? { "content-type": "application/json" } : {}),
        ...windowIdentityHeader(),
      },
      // Cookie transport: send/receive HTTP-only session cookie on every
      // request. Replaces localStorage token storage (M2).
      credentials: "include",
    };
    if (method === "POST" && body !== undefined) {
      init.body = JSON.stringify(body);
    }
    response = await fetch(`${API_URL}${path}`, init);
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
  return requestJson<SignUpResult>("/accounts", "POST", input);
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  return requestJson<SignInResult>("/sessions", "POST", input);
}

export function acceptInvitation(
  token: string,
  input: Omit<AcceptInvitationInput, "invitation_token">,
): Promise<AcceptInvitationResult> {
  const path = `/invitations/${encodeURIComponent(token)}/accept`;
  return requestJson<AcceptInvitationResult>(path, "POST", {
    email: input.email,
    password: input.password,
    display_name: input.display_name,
  });
}

export function listActiveAccounts(): Promise<ListActiveAccountsResult> {
  return requestJson<ListActiveAccountsResult>("/accounts/active", "GET");
}

export function switchActiveAccount(
  input: SwitchActiveAccountInput,
): Promise<SwitchActiveAccountResult> {
  return requestJson<SwitchActiveAccountResult>(
    "/accounts/active/switch",
    "POST",
    input,
  );
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
      headers: windowIdentityHeader(),
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

function windowIdentityHeader(): Record<string, string> {
  const windowId = getWindowId();
  if (windowId === null) {
    return {};
  }
  return { [WINDOW_ID_HEADER]: windowId };
}

function getWindowId(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    const existing = window.sessionStorage.getItem(WINDOW_ID_STORAGE_KEY);
    if (existing !== null && existing.trim() !== "") {
      return existing;
    }
    const created =
      typeof globalThis.crypto?.randomUUID === "function"
        ? globalThis.crypto.randomUUID()
        : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    window.sessionStorage.setItem(WINDOW_ID_STORAGE_KEY, created);
    return created;
  } catch {
    return null;
  }
}
