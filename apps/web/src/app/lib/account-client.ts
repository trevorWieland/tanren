import * as v from "valibot";

import {
  signUpRequestSchema,
  signUpResponseSchema,
  signInRequestSchema,
  signInResponseSchema,
  acceptInvitationRequestSchema,
  acceptInvitationResponseSchema,
} from "@/app/lib/api-contract-valibot.gen";
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

function parseAccountResponse<
  TSchema extends v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
>(schema: TSchema, payload: unknown): v.InferOutput<TSchema> {
  const result = v.safeParse(schema, payload);
  if (result.success) {
    return result.output;
  }
  throw new AccountRequestError({
    code: "internal_error",
    summary: "Response body does not match account contract.",
  });
}

function parseAccountInput<
  TSchema extends v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
>(schema: TSchema, payload: unknown): v.InferOutput<TSchema> {
  const result = v.safeParse(schema, payload);
  if (result.success) {
    return result.output;
  }
  throw new AccountRequestError({
    code: "validation_failed",
    summary: "Request body does not match account contract.",
  });
}

async function postJson<
  TSchema extends v.BaseSchema<unknown, unknown, v.BaseIssue<unknown>>,
>(
  path: string,
  body: unknown,
  schema: TSchema,
): Promise<v.InferOutput<TSchema>> {
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

  let payload: unknown;
  try {
    payload = (await response.json()) as unknown;
  } catch {
    throw new AccountRequestError({
      code: "internal_error",
      summary: `HTTP ${response.status}`,
    });
  }
  return parseAccountResponse(schema, payload);
}

export function signUp(input: SignUpInput): Promise<SignUpResult> {
  const parsedInput = parseAccountInput(signUpRequestSchema, input);
  return postJson(accountApiPaths.signUp, parsedInput, signUpResponseSchema);
}

export function signIn(input: SignInInput): Promise<SignInResult> {
  const parsedInput = parseAccountInput(signInRequestSchema, input);
  return postJson(accountApiPaths.signIn, parsedInput, signInResponseSchema);
}

export function acceptInvitation(
  token: string,
  input: AcceptInvitationInput,
): Promise<AcceptInvitationResult> {
  const pathTemplate = accountApiPaths.acceptInvitation;
  const path = pathTemplate.replace("{token}", encodeURIComponent(token));
  const parsedInput = parseAccountInput(acceptInvitationRequestSchema, {
    email: input.email,
    password: input.password,
    display_name: input.display_name,
  });
  return postJson(path, parsedInput, acceptInvitationResponseSchema);
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
