import * as m from "@/i18n/paraglide/messages";
import * as v from "valibot";
import type {
  AccountFailureCode,
  AccountId,
  AccountView,
  Brand,
  ListActiveAccountsResponse,
  OrgId,
  SignedInAccountView,
  SwitchActiveAccountRequest,
  SwitchActiveAccountResponse,
} from "@/app/lib/generated/account-contract";
import { asAccountId, asOrgId } from "@/app/lib/generated/account-contract";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";
const WINDOW_ID_HEADER = "x-tanren-window-id";
const WINDOW_ID_STORAGE_KEY = "tanren.window_id";
type WindowContextId = Brand<string, "WindowContextId">;

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

const AccountIdSchema = v.pipe(
  v.string(),
  v.trim(),
  v.uuid(),
  v.transform((value): AccountId => asAccountId(value)),
);

const OrgIdSchema = v.pipe(
  v.string(),
  v.trim(),
  v.uuid(),
  v.transform((value): OrgId => asOrgId(value)),
);

const WindowContextIdSchema = v.pipe(
  v.string(),
  v.trim(),
  v.uuid(),
  v.transform((value): WindowContextId => value as WindowContextId),
);

const AccountViewSchema = v.object({
  id: AccountIdSchema,
  identifier: v.string(),
  display_name: v.string(),
  org: v.nullable(OrgIdSchema),
});

const SignedInAccountViewSchema = v.object({
  account: AccountViewSchema,
  is_active: v.boolean(),
});

const SessionViewSchema = v.object({
  account_id: AccountIdSchema,
  expires_at: v.string(),
});

const SignUpResultSchema = v.object({
  account: AccountViewSchema,
  session: SessionViewSchema,
});

const SignInResultSchema = v.object({
  account: AccountViewSchema,
  session: SessionViewSchema,
});

const AcceptInvitationResultSchema = v.object({
  account: AccountViewSchema,
  session: SessionViewSchema,
  joined_org: OrgIdSchema,
});

const ListActiveAccountsResponseSchema = v.object({
  accounts: v.array(SignedInAccountViewSchema),
});

const SwitchActiveAccountResponseSchema = v.object({
  active_account_id: AccountIdSchema,
  accounts: v.array(SignedInAccountViewSchema),
});

const FailureBodySchema = v.object({
  code: v.optional(v.string()),
  summary: v.optional(v.string()),
});

type JsonDecoder<T> = (payload: unknown) => T | null;

function decodeWithSchema<
  TSchema extends v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
>(schema: TSchema, payload: unknown): v.InferOutput<TSchema> | null {
  const parsed = v.safeParse(schema, payload);
  if (!parsed.success) {
    return null;
  }
  return parsed.output;
}

function decodeAccountId(payload: unknown): AccountId | null {
  return decodeWithSchema(AccountIdSchema, payload);
}

function decodeWindowContextId(payload: unknown): WindowContextId | null {
  return decodeWithSchema(WindowContextIdSchema, payload);
}

function decodeFailureBody(payload: unknown): FailureBody | null {
  return decodeWithSchema(FailureBodySchema, payload);
}

function decodeSignUpResult(payload: unknown): SignUpResult | null {
  return decodeWithSchema(SignUpResultSchema, payload);
}

function decodeSignInResult(payload: unknown): SignInResult | null {
  return decodeWithSchema(SignInResultSchema, payload);
}

function decodeAcceptInvitationResult(
  payload: unknown,
): AcceptInvitationResult | null {
  return decodeWithSchema(AcceptInvitationResultSchema, payload);
}

function decodeListActiveAccountsResult(
  payload: unknown,
): ListActiveAccountsResult | null {
  return decodeWithSchema(ListActiveAccountsResponseSchema, payload);
}

function decodeSwitchActiveAccountResult(
  payload: unknown,
): SwitchActiveAccountResult | null {
  return decodeWithSchema(SwitchActiveAccountResponseSchema, payload);
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
  decode: JsonDecoder<T>,
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
    let parsed: FailureBody | null = null;
    try {
      parsed = decodeFailureBody(await response.json());
    } catch {
      parsed = null;
    }
    const code =
      typeof parsed?.code === "string" ? parsed.code : "internal_error";
    const summary =
      typeof parsed?.summary === "string"
        ? parsed.summary
        : `HTTP ${response.status}`;
    throw new AccountRequestError({ code, summary });
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

  const decoded = decode(payload);
  if (decoded === null) {
    throw new AccountRequestError({
      code: "internal_error",
      summary: "Invalid response body.",
    });
  }
  return decoded;
}

export function signUp(input: SignUpInput): Promise<SignUpResult> {
  return requestJson("/accounts", "POST", decodeSignUpResult, input);
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  return requestJson("/sessions", "POST", decodeSignInResult, input);
}

export function acceptInvitation(
  token: string,
  input: Omit<AcceptInvitationInput, "invitation_token">,
): Promise<AcceptInvitationResult> {
  const path = `/invitations/${encodeURIComponent(token)}/accept`;
  return requestJson(path, "POST", decodeAcceptInvitationResult, {
    email: input.email,
    password: input.password,
    display_name: input.display_name,
  });
}

export function listActiveAccounts(): Promise<ListActiveAccountsResult> {
  return requestJson("/accounts/active", "GET", decodeListActiveAccountsResult);
}

export function switchActiveAccount(
  input: SwitchActiveAccountInput,
): Promise<SwitchActiveAccountResult> {
  return requestJson(
    "/accounts/active/switch",
    "POST",
    decodeSwitchActiveAccountResult,
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

export function parseAccountId(value: string): AccountId | null {
  return decodeAccountId(value);
}

function windowIdentityHeader(): Record<string, string> {
  const windowId = getWindowId();
  if (windowId === null) {
    return {};
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
      const existing = decodeWindowContextId(existingRaw);
      if (existing !== null) {
        return existing;
      }
      window.sessionStorage.removeItem(WINDOW_ID_STORAGE_KEY);
    }

    if (typeof globalThis.crypto?.randomUUID !== "function") {
      return null;
    }
    const created = decodeWindowContextId(globalThis.crypto.randomUUID());
    if (created === null) {
      return null;
    }
    window.sessionStorage.setItem(WINDOW_ID_STORAGE_KEY, created);
    return created;
  } catch {
    return null;
  }
}
