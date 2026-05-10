import * as m from "@/i18n/paraglide/messages";
import type {
  AcceptInvitationRequest,
  AccountFailureCode,
  AccountRequestFailureCode,
  AccountId,
  AccountView,
  OrgId,
  SessionEnvelope,
  SignedInAccountView,
  SwitchActiveAccountRequest,
  SwitchActiveAccountResponse,
  WindowContextId,
} from "@/app/lib/generated/account-contract";
import {
  parseAccountId as parseGeneratedAccountId,
  parseListActiveAccountsResponse,
  parseSwitchActiveAccountResponse,
  parseWebAcceptInvitationResponse,
  parseWebSignInResponse,
  parseWebSignUpResponse,
} from "@/app/lib/generated/account-contract";
import { getJson, postEmpty, postJson } from "@/app/lib/account-http";
import {
  browserWindowContext,
  type WindowContextStrategy,
} from "@/app/lib/window-context";
import {
  getCachedWindowId as getCachedWindowIdImpl,
  invalidateCachedWindowId as invalidateCachedWindowIdImpl,
  parseWindowContextId as parseWindowContextIdImpl,
} from "@/app/lib/window-context";

/** Maximum number of signed-in accounts the server and validators accept. */
export const ACTIVE_ACCOUNT_LIMIT = 16;

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

export type ListActiveAccountsResult =
  import("@/app/lib/generated/account-contract").ListActiveAccountsResponse;
export type SwitchActiveAccountInput = SwitchActiveAccountRequest;
export type SwitchActiveAccountResult = SwitchActiveAccountResponse;

export interface AccountFailure {
  code: AccountRequestFailureCode;
  summary: string;
}

// -- Session decoding (shared contract-consumption pattern) --

function parseCookieSessionExpiry(value: string): Date | null {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }
  return parsed;
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

function decodeSignUpResult(payload: unknown): SignUpResult | null {
  const decoded = parseWebSignUpResponse(payload);
  if (decoded === null) {
    return null;
  }
  const session = decodeCookieSessionEnvelope(decoded.session);
  if (session === null) {
    return null;
  }
  return { account: decoded.account, session };
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
  return { account: decoded.account, session };
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
  return { account: decoded.account, session, joined_org: decoded.joined_org };
}

// -- Auth-side-effect wrapper --

/**
 * Execute an operation that produces a result, then rotate the window
 * context as an explicit authentication success side effect.
 */
async function withAuthRotate<TResult>(
  operation: () => Promise<TResult>,
  windowCtx: WindowContextStrategy,
): Promise<TResult> {
  const result = await operation();
  windowCtx.rotate();
  return result;
}

// -- Public API --

export function signUp(input: SignUpInput): Promise<SignUpResult> {
  return withAuthRotate(
    () =>
      postJson("/accounts", input, decodeSignUpResult, browserWindowContext),
    browserWindowContext,
  );
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  return withAuthRotate(
    () =>
      postJson("/sessions", input, decodeSignInResult, browserWindowContext),
    browserWindowContext,
  );
}

export function acceptInvitation(
  token: string,
  input: Omit<AcceptInvitationInput, "invitation_token">,
): Promise<AcceptInvitationResult> {
  const request = {
    display_name: input.display_name,
    email: input.email,
    invitation_token: token,
    password: input.password,
  };
  return withAuthRotate(
    () =>
      postJson(
        (req: AcceptInvitationRequest) =>
          `/invitations/${encodeURIComponent(req.invitation_token)}/accept`,
        request,
        decodeAcceptInvitationResult,
        browserWindowContext,
        (req) => ({
          display_name: req.display_name,
          email: req.email,
          password: req.password,
        }),
      ),
    browserWindowContext,
  );
}

export function listActiveAccounts(): Promise<ListActiveAccountsResult> {
  return getJson(
    "/accounts/active",
    parseListActiveAccountsResponse,
    browserWindowContext,
  );
}

export function switchActiveAccount(
  input: SwitchActiveAccountInput,
): Promise<SwitchActiveAccountResult> {
  return postJson(
    "/accounts/active/switch",
    input,
    parseSwitchActiveAccountResponse,
    browserWindowContext,
  );
}

/**
 * Sign-out clears the session row server-side and the cookie via
 * `Set-Cookie: tanren_session=; Max-Age=0`.
 */
export async function signOut(): Promise<void> {
  await postEmpty("/sessions/revoke", {}, browserWindowContext);
  browserWindowContext.clear();
}

// -- Bounded keyed lookup helpers --

/**
 * Extract the active account id from a signed-in account list using a
 * bounded scan. Returns `null` when no entry is marked active.
 */
export function getActiveAccountId(
  accounts: readonly SignedInAccountView[],
): AccountId | null {
  for (let i = 0; i < accounts.length && i < ACTIVE_ACCOUNT_LIMIT; i++) {
    if (accounts[i]!.is_active) {
      return accounts[i]!.account.id;
    }
  }
  return null;
}

/**
 * Find a specific signed-in account by its account id using bounded
 * keyed lookup. Returns `null` when no matching entry exists.
 */
export function findSignedInAccount(
  accounts: readonly SignedInAccountView[],
  accountId: AccountId,
): SignedInAccountView | null {
  for (let i = 0; i < accounts.length && i < ACTIVE_ACCOUNT_LIMIT; i++) {
    if (accounts[i]!.account.id === accountId) {
      return accounts[i]!;
    }
  }
  return null;
}

/**
 * Build a `Map<AccountId, SignedInAccountView>` from a bounded account
 * list for O(1) keyed lookups within a single request lifecycle. The
 * map is scoped to the bounded limit so callers never index an
 * unbounded structure.
 */
export function buildAccountLookup(
  accounts: readonly SignedInAccountView[],
): Map<AccountId, SignedInAccountView> {
  const map = new Map<AccountId, SignedInAccountView>();
  for (let i = 0; i < accounts.length && i < ACTIVE_ACCOUNT_LIMIT; i++) {
    const entry = accounts[i]!;
    map.set(entry.account.id, entry);
  }
  return map;
}

// -- Re-exports from sub-modules --

export const getCachedWindowId = getCachedWindowIdImpl;
export const invalidateCachedWindowId = invalidateCachedWindowIdImpl;

export function parseAccountId(value: string): AccountId | null {
  return parseGeneratedAccountId(value);
}

export function parseWindowContextId(payload: unknown): WindowContextId | null {
  return parseWindowContextIdImpl(payload);
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
