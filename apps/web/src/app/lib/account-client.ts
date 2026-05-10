import {
  accountApiPaths,
  accountFailureCodes,
  type AcceptInvitationInput,
  type AcceptInvitationResult,
  type AccountFailure,
  type SignInInput,
  type SignInResult,
  type SignUpInput,
  type SignUpResult,
} from "@/app/lib/contracts";
import {
  parseFailureResponse,
  renderFailureEnvelope,
  unavailableFailure,
  type FailureEnvelope,
} from "@/app/lib/failure";

const API_URL = process.env["NEXT_PUBLIC_API_URL"] ?? "http://localhost:8080";

export type {
  AcceptInvitationInput,
  AcceptInvitationResult,
  AccountFailure,
  AccountView,
  SessionView,
  SignInInput,
  SignInResult,
  SignUpInput,
  SignUpResult,
} from "@/app/lib/contracts";

/**
 * Map an `AccountFailure` to a localized message via paraglide. Falls back
 * to the API-supplied summary, then to a generic "Request failed" string,
 * so unknown failure codes still surface something meaningful.
 */
export function describeFailure(failure: AccountFailure): string {
  return renderFailureEnvelope(failure);
}

export class AccountRequestError extends Error {
  readonly failure: AccountFailure;

  constructor(failure: AccountFailure) {
    super(describeFailure(failure));
    this.failure = failure;
    this.name = "AccountRequestError";
  }
}

function isAccountFailureCode(code: string): code is AccountFailure["code"] {
  return (accountFailureCodes as readonly string[]).includes(code);
}

function toAccountFailure(failure: FailureEnvelope): AccountFailure {
  if (isAccountFailureCode(failure.code)) {
    return {
      code: failure.code,
      summary: failure.summary,
    };
  }
  return {
    code: "internal_error",
    summary: failure.summary || "Request failed.",
  };
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
    throw new AccountRequestError(toAccountFailure(unavailableFailure(cause)));
  }

  if (!response.ok) {
    const failureEnvelope = await parseFailureResponse(response);
    throw new AccountRequestError(toAccountFailure(failureEnvelope));
  }

  return (await response.json()) as T;
}

export function signUp(input: SignUpInput): Promise<SignUpResult> {
  return postJson<SignUpResult>(accountApiPaths.signUp, input);
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  return postJson<SignInResult>(accountApiPaths.signIn, input);
}

export function acceptInvitation(
  token: string,
  input: AcceptInvitationInput,
): Promise<AcceptInvitationResult> {
  const pathTemplate = accountApiPaths.acceptInvitation;
  const path = pathTemplate.replace("{token}", encodeURIComponent(token));
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
    response = await fetch(`${API_URL}${accountApiPaths.revokeSession}`, {
      method: "POST",
      credentials: "include",
    });
  } catch (cause: unknown) {
    throw new AccountRequestError(toAccountFailure(unavailableFailure(cause)));
  }
  if (!response.ok) {
    const failureEnvelope = await parseFailureResponse(response);
    throw new AccountRequestError(toAccountFailure(failureEnvelope));
  }
}
