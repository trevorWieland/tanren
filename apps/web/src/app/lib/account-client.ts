const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080";

const SESSION_TOKEN_KEY = "tanren.session_token";
const ACCOUNT_ID_KEY = "tanren.account_id";

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

export interface SessionView {
  account_id: string;
  token: string;
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

const FAILURE_MESSAGES: Record<AccountFailureCode, string> = {
  duplicate_identifier: "An account already exists for that email.",
  invalid_credential: "Email or password is invalid.",
  invitation_not_found: "This invitation link is not recognized.",
  invitation_already_consumed: "This invitation has already been accepted.",
  invitation_expired: "This invitation has expired.",
  validation_failed: "Please check the form fields and try again.",
  unavailable: "Tanren is temporarily unavailable. Please try again shortly.",
  internal_error: "Something went wrong. Please try again.",
};

export function describeFailure(failure: AccountFailure): string {
  const known = FAILURE_MESSAGES[failure.code as AccountFailureCode];
  if (known !== undefined) {
    return known;
  }
  return failure.summary !== "" ? failure.summary : "Request failed.";
}

export function persistSession(accountId: string, sessionToken: string): void {
  if (typeof window === "undefined") {
    return;
  }
  window.localStorage.setItem(SESSION_TOKEN_KEY, sessionToken);
  window.localStorage.setItem(ACCOUNT_ID_KEY, accountId);
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
    password: input.password,
    display_name: input.display_name,
  });
}
