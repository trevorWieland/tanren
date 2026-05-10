import * as m from "@/i18n/paraglide/messages";
import type {
  AcceptInvitationRequest,
  AccountFailureCode,
  AccountRequestFailureCode,
  AccountId,
  AccountView,
  ListActiveAccountsRequest,
  ListActiveAccountsResponse,
  OrgId,
  SessionEnvelope,
  SignInRequest,
  SignUpRequest,
  SignedInAccountView,
  SwitchActiveAccountRequest,
  SwitchActiveAccountResponse,
  WindowContextId,
} from "@/app/lib/generated/account-contract";
import {
  parseAcceptInvitationRequest,
  parseAccountId as parseGeneratedAccountId,
  parseAccountRequestFailureCode,
  parseListActiveAccountsRequest,
  parseListActiveAccountsResponse,
  parseSignInRequest,
  parseSignUpRequest,
  parseSwitchActiveAccountRequest,
  parseSwitchActiveAccountResponse,
  parseWebAcceptInvitationResponse,
  parseWebSignInResponse,
  parseWebSignUpResponse,
  parseWindowContextId as parseGeneratedWindowContextId,
} from "@/app/lib/generated/account-contract";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";
const WINDOW_ID_HEADER = "x-tanren-window-id";
const WINDOW_ID_STORAGE_KEY = "tanren.window_id";
const WINDOW_ID_FAILURE_SUMMARY =
  "Unable to initialize browser window context.";

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
  expires_at: Date;
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

export interface AccountFailure {
  code: AccountRequestFailureCode;
  summary: string;
}

interface FailureBody {
  code?: unknown;
  summary?: unknown;
}

type JsonDecoder<T> = (payload: unknown) => T | null;

type ResponseMode = "json" | "empty";

type AccountOperation<TRequest, TResponse> = {
  method: "GET" | "POST";
  path: string | ((request: TRequest) => string);
  parseRequest: JsonDecoder<TRequest>;
  responseMode: ResponseMode;
  parseResponse?: JsonDecoder<TResponse>;
  encodeBody?: (request: TRequest) => unknown;
  afterSuccess?: () => void;
};

const ACCOUNT_OPERATIONS: {
  signUp: AccountOperation<SignUpRequest, SignUpResult>;
  signIn: AccountOperation<SignInRequest, SignInResult>;
  acceptInvitation: AccountOperation<
    AcceptInvitationRequest,
    AcceptInvitationResult
  >;
  listActiveAccounts: AccountOperation<
    ListActiveAccountsRequest,
    ListActiveAccountsResult
  >;
  switchActiveAccount: AccountOperation<
    SwitchActiveAccountRequest,
    SwitchActiveAccountResult
  >;
  signOut: AccountOperation<ListActiveAccountsRequest, void>;
} = {
  signUp: {
    method: "POST",
    path: "/accounts",
    parseRequest: parseSignUpRequest,
    responseMode: "json",
    parseResponse: decodeSignUpResult,
    afterSuccess: rotateWindowId,
  },
  signIn: {
    method: "POST",
    path: "/sessions",
    parseRequest: parseSignInRequest,
    responseMode: "json",
    parseResponse: decodeSignInResult,
    afterSuccess: rotateWindowId,
  },
  acceptInvitation: {
    method: "POST",
    path: (request) =>
      `/invitations/${encodeURIComponent(request.invitation_token)}/accept`,
    parseRequest: parseAcceptInvitationRequest,
    responseMode: "json",
    parseResponse: decodeAcceptInvitationResult,
    encodeBody: (request) => ({
      display_name: request.display_name,
      email: request.email,
      password: request.password,
    }),
    afterSuccess: rotateWindowId,
  },
  listActiveAccounts: {
    method: "GET",
    path: "/accounts/active",
    parseRequest: parseListActiveAccountsRequest,
    responseMode: "json",
    parseResponse: parseListActiveAccountsResponse,
  },
  switchActiveAccount: {
    method: "POST",
    path: "/accounts/active/switch",
    parseRequest: parseSwitchActiveAccountRequest,
    responseMode: "json",
    parseResponse: parseSwitchActiveAccountResponse,
  },
  signOut: {
    method: "POST",
    path: "/sessions/revoke",
    parseRequest: parseListActiveAccountsRequest,
    responseMode: "empty",
    afterSuccess: clearWindowId,
  },
};

function decodeFailureBody(payload: unknown): FailureBody | null {
  if (
    payload === null ||
    typeof payload !== "object" ||
    Array.isArray(payload)
  ) {
    return null;
  }
  const source = payload as Record<string, unknown>;
  return {
    code: source["code"],
    summary: source["summary"],
  };
}

function parseCookieSessionExpiry(value: string): Date | null {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }
  return parsed;
}

function decodeSignUpResult(payload: unknown): SignUpResult | null {
  const decoded = parseWebSignUpResponse(payload);
  if (decoded === null) {
    return null;
  }
  const session = decodeCookieSessionEnvelope(decoded.session);
  if (session === null) {
    return null;
  }
  return {
    account: decoded.account,
    session,
  };
}

function decodeSignInResult(payload: unknown): SignInResult | null {
  const decoded = parseWebSignInResponse(payload);
  if (decoded === null) {
    return null;
  }
  const session = decodeCookieSessionEnvelope(decoded.session);
  if (session === null) {
    return null;
  }
  return {
    account: decoded.account,
    session,
  };
}

function decodeAcceptInvitationResult(
  payload: unknown,
): AcceptInvitationResult | null {
  const decoded = parseWebAcceptInvitationResponse(payload);
  if (decoded === null) {
    return null;
  }
  const session = decodeCookieSessionEnvelope(decoded.session);
  if (session === null) {
    return null;
  }
  return {
    account: decoded.account,
    session,
    joined_org: decoded.joined_org,
  };
}

function decodeCookieSessionEnvelope(
  payload: SessionEnvelope,
): SessionView | null {
  if (payload.transport !== "cookie") {
    return null;
  }
  const expiresAt = parseCookieSessionExpiry(payload.expires_at);
  if (expiresAt === null) {
    return null;
  }
  return {
    account_id: payload.account_id,
    expires_at: expiresAt,
  };
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

async function requestJson<TRequest, TResponse>(
  operation: AccountOperation<TRequest, TResponse>,
  input: unknown,
): Promise<TResponse> {
  const request = operation.parseRequest(input);
  if (request === null) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: "Invalid request body.",
    });
  }

  const identityHeader = windowIdentityHeader();
  let response: Response;
  try {
    const init: RequestInit = {
      method: operation.method,
      headers: {
        ...(operation.method === "POST"
          ? { "content-type": "application/json" }
          : {}),
        ...identityHeader,
      },
      // Cookie transport: send/receive HTTP-only session cookie on every
      // request. Replaces localStorage token storage (M2).
      credentials: "include",
    };
    if (operation.method === "POST") {
      const body = operation.encodeBody?.(request) ?? request;
      if (body !== undefined) {
        init.body = JSON.stringify(body);
      }
    }
    const path =
      typeof operation.path === "function"
        ? operation.path(request)
        : operation.path;
    response = await fetch(`${API_URL}${path}`, init);
  } catch (cause: unknown) {
    throw new AccountRequestError({
      code: "unavailable",
      summary: cause instanceof Error ? cause.message : String(cause),
    });
  }

  if (!response.ok) {
    let parsed: FailureBody | null = null;
    if (response.status !== 204) {
      try {
        parsed = decodeFailureBody(await response.json());
      } catch {
        parsed = null;
      }
    }
    const code =
      parseAccountRequestFailureCode(parsed?.code) ?? "internal_error";
    const summary =
      typeof parsed?.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    if (windowContextRejectedByServer(code, summary)) {
      rotateWindowId();
    }
    throw new AccountRequestError({ code, summary });
  }

  if (operation.responseMode === "empty") {
    operation.afterSuccess?.();
    return undefined as TResponse;
  }

  let payload: unknown;
  try {
    payload = await response.json();
  } catch {
    throw new AccountRequestError({
      code: "internal_error",
      summary: "Invalid response body.",
    });
  }

  const decode = operation.parseResponse;
  if (decode === undefined) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: "Invalid response decoder.",
    });
  }
  const decoded = decode(payload);
  if (decoded === null) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: "Invalid response body.",
    });
  }
  operation.afterSuccess?.();
  return decoded;
}

export function signUp(input: SignUpInput): Promise<SignUpResult> {
  return requestJson(ACCOUNT_OPERATIONS.signUp, input);
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  return requestJson(ACCOUNT_OPERATIONS.signIn, input);
}

export function acceptInvitation(
  token: string,
  input: Omit<AcceptInvitationInput, "invitation_token">,
): Promise<AcceptInvitationResult> {
  return requestJson(ACCOUNT_OPERATIONS.acceptInvitation, {
    display_name: input.display_name,
    email: input.email,
    invitation_token: token,
    password: input.password,
  });
}

export function listActiveAccounts(): Promise<ListActiveAccountsResult> {
  return requestJson(ACCOUNT_OPERATIONS.listActiveAccounts, {});
}

export function switchActiveAccount(
  input: SwitchActiveAccountInput,
): Promise<SwitchActiveAccountResult> {
  return requestJson(ACCOUNT_OPERATIONS.switchActiveAccount, input);
}

/**
 * Sign-out clears the session row server-side and the cookie via
 * `Set-Cookie: tanren_session=; Max-Age=0`.
 */
export function signOut(): Promise<void> {
  return requestJson(ACCOUNT_OPERATIONS.signOut, {});
}

export function parseAccountId(value: string): AccountId | null {
  return parseGeneratedAccountId(value);
}

export function parseWindowContextId(payload: unknown): WindowContextId | null {
  return parseGeneratedWindowContextId(payload);
}

function windowIdentityHeader(): Record<string, string> {
  const windowId = getWindowId();
  if (windowId === null) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: WINDOW_ID_FAILURE_SUMMARY,
    });
  }
  return { [WINDOW_ID_HEADER]: windowId };
}

function getWindowId(): WindowContextId | null {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    const existingRaw = window.sessionStorage.getItem(WINDOW_ID_STORAGE_KEY);
    if (existingRaw !== null) {
      const existing = parseWindowContextId(existingRaw);
      if (existing !== null) {
        return existing;
      }
      clearWindowId();
    }

    if (typeof globalThis.crypto?.randomUUID !== "function") {
      return null;
    }
    const created = parseWindowContextId(globalThis.crypto.randomUUID());
    if (created === null) {
      return null;
    }
    window.sessionStorage.setItem(WINDOW_ID_STORAGE_KEY, created);
    return created;
  } catch {
    return null;
  }
}

function clearWindowId(): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.sessionStorage.removeItem(WINDOW_ID_STORAGE_KEY);
  } catch {
    // Ignore storage access errors; callers already surface failures.
  }
}

function rotateWindowId(): void {
  clearWindowId();
  // Best-effort replacement so the next request can proceed without
  // waiting for a second retry path.
  void getWindowId();
}

function windowContextRejectedByServer(
  code: AccountRequestFailureCode,
  summary: string,
): boolean {
  if (code !== "validation_failed") {
    return false;
  }
  const normalized = summary.toLowerCase();
  return (
    normalized.includes("window id") || normalized.includes(WINDOW_ID_HEADER)
  );
}
